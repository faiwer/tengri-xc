/** Finds the <a href/> link that includes `path` in `body`. */
export function findLink(body: string, path: string): string {
  const hrefs = [...body.matchAll(/href="([^"]+)"/g)]
    .map((match) => match[1])
    .filter((href) => href.includes(path));

  if (hrefs.length !== 1) {
    throw new Error(
      `Expected one link to ${path}, found ${hrefs.length}:\n${body}`,
    );
  }
  return hrefs[0];
}
