use std::env;
use std::error::Error;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex};
use std::thread;

use tengri_maps::geo::XyzTile;
use tengri_maps::serve::{TileServeFormat, tile_serve_format};
use tengri_maps::tree::{TileTreeError, TileTreeReader};

const DEFAULT_ADDR: &str = "127.0.0.1:8088";

struct TileServer {
    tree: Mutex<TileTreeReader>,
    serve_format: Box<dyn TileServeFormat>,
}

fn main() {
    let mut args = env::args_os().skip(1);
    let first = args.next();
    if first.as_deref() == Some(std::ffi::OsStr::new("--help"))
        || first.as_deref() == Some(std::ffi::OsStr::new("-h"))
    {
        print_usage();
        return;
    }

    let Some(tree_path) = first.map(PathBuf::from) else {
        print_usage();
        process::exit(2);
    };
    let addr = args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .unwrap_or_else(|| DEFAULT_ADDR.to_owned());

    if args.next().is_some() {
        print_usage();
        process::exit(2);
    }

    let tree = match TileTreeReader::open(&tree_path) {
        Ok(reader) => reader,
        Err(error) => {
            eprintln!("failed to open tile tree {}: {error}", tree_path.display());
            process::exit(1);
        }
    };
    let tile_kind = tree.metadata().tile_kind;
    let serve_format = tile_serve_format(tile_kind);

    let listener = match TcpListener::bind(&addr) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("failed to bind {addr}: {error}");
            process::exit(1);
        }
    };

    eprintln!(
        "serving {:?} tiles from {} on http://{addr}",
        tile_kind,
        tree_path.display(),
    );
    let server = Arc::new(TileServer {
        tree: Mutex::new(tree),
        serve_format,
    });

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let server = Arc::clone(&server);
                thread::spawn(move || {
                    if let Err(error) = serve(stream, &server) {
                        eprintln!("request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("connection failed: {error}"),
        }
    }
}

fn serve(mut stream: TcpStream, server: &TileServer) -> Result<(), Box<dyn Error>> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let Some(path) = request_path(&request_line) else {
        return write_response(
            &mut stream,
            400,
            "Bad Request",
            "text/plain",
            b"bad request",
        );
    };

    let Some(tile) = server.serve_format.parse_path(path) else {
        return write_response(&mut stream, 404, "Not Found", "text/plain", b"not found");
    };

    let Some(payload) = read_tree_tile(server, tile)? else {
        return write_response(
            &mut stream,
            404,
            "Not Found",
            "text/plain",
            b"tile not found",
        );
    };
    let tile = server.serve_format.render(&payload)?;
    write_response(&mut stream, 200, "OK", tile.content_type, &tile.body)
}

fn read_tree_tile(server: &TileServer, tile: XyzTile) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
    let Ok(lng) = u16::try_from(tile.x) else {
        return Ok(None);
    };
    let Ok(lat) = u16::try_from(tile.y) else {
        return Ok(None);
    };

    let mut tree = server
        .tree
        .lock()
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "tile tree reader lock is poisoned"))?;

    match tree.read(tile.z, lng, lat) {
        Ok(tile) => Ok(Some(tile)),
        Err(error) if tree_error_is_not_found(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn tree_error_is_not_found(error: &TileTreeError) -> bool {
    matches!(
        error,
        TileTreeError::MissingTile { .. } | TileTreeError::TileOutOfBounds { .. }
    )
}

/// E.g. `GET /terrain/8/123/45.png HTTP/1.1` -> `/terrain/8/123/45.png`.
fn request_path(request_line: &str) -> Option<&str> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    if method != "GET" {
        return None;
    }
    Some(path)
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> Result<(), Box<dyn Error>> {
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}

fn print_usage() {
    eprintln!("usage: serve_tiles <tile-tree> [addr]");
    eprintln!("default addr: {DEFAULT_ADDR}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tengri_maps::tree::TileKind;

    #[test]
    fn parses_terrain_png_path() {
        let serve_format = tile_serve_format(TileKind::Dem);
        assert_eq!(
            serve_format.parse_path("/terrain/8/123/45.png"),
            Some(XyzTile {
                z: 8,
                x: 123,
                y: 45
            })
        );
    }

    #[test]
    fn tree_missing_errors_are_not_found() {
        assert!(tree_error_is_not_found(&TileTreeError::MissingTile {
            z: 8,
            x: 123,
            y: 45,
        }));
        assert!(tree_error_is_not_found(&TileTreeError::TileOutOfBounds {
            z: 8,
            x: 123,
            y: 45,
        }));
        assert!(!tree_error_is_not_found(&TileTreeError::CorruptFile("bad")));
    }
}
