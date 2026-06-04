import clsx from 'clsx';
import { useMemo, useRef, useState, type ReactNode } from 'react';
import styles from './DropZone.module.scss';

interface DropZoneProps {
  /** The list of valid file extensions. */
  extensions: string[];
  /** Whether the drop zone accepts exactly one file or many. */
  mode: DropZoneMode;
  /** Shown during dragging when the dragging content is invalid. */
  invalidContent: ReactNode;
  /** Shown in the drag zone before dragging or when the dnd state is valid. */
  children: ReactNode;
  /** Called when files are dropped (DnD is done). */
  onDropFiles: (files: File[]) => void;
}

export function DropZone({
  extensions,
  mode,
  invalidContent,
  children,
  onDropFiles,
}: DropZoneProps) {
  const acceptedExtensions = useMemo(
    () =>
      new Set(extensions.map((ext) => ext.replace(/^\./, '').toLowerCase())),
    [extensions],
  );
  const [state, setState] = useState<DropZoneState>('idle');
  const dragDepth = useRef(0);

  const onDragOver = (event: DragEvent) => {
    const { dataTransfer } = event;
    if (!dataTransfer || !isFileDrag(dataTransfer)) {
      return;
    }

    // Tell the browser this node can accept the drop (receive `onDrop`).
    event.preventDefault();
    event.stopPropagation();

    const files = filesFrom(dataTransfer);
    // Chances `files` are not empty are tiny. We have `items` here, but they
    // don't contain extensions, only mime types. Mime types for .IGC files are
    // empty strings. So we can truly validate the files only in `onDrop`.
    const nextState =
      !files || areFilesAccepted(files, acceptedExtensions, mode)
        ? 'valid'
        : 'invalid';
    dataTransfer.dropEffect = nextState === 'valid' ? 'copy' : 'none';
    setState(nextState);
  };

  const onDragEnter = (event: DragEvent) => {
    const { dataTransfer } = event;
    if (dataTransfer && isFileDrag(dataTransfer)) {
      dragDepth.current += 1;
    }
    onDragOver(event);
  };

  const onDragLeave = (event: DragEvent) => {
    const { dataTransfer } = event;
    if (!dataTransfer || !isFileDrag(dataTransfer)) {
      return;
    }

    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) {
      setState('idle');
    }
  };

  const onDrop = (event: DragEvent) => {
    const { dataTransfer } = event;
    if (!dataTransfer || !isFileDrag(dataTransfer)) {
      return;
    }

    // Finish the drag operation.
    event.preventDefault();
    event.stopPropagation();
    dragDepth.current = 0;

    const files = filesFrom(dataTransfer);
    if (!files || !areFilesAccepted(files, acceptedExtensions, mode)) {
      dataTransfer.dropEffect = 'none'; // Might change the cursor.
      setState('invalid');
      return;
    }

    setState('idle');
    onDropFiles(files);
  };

  return (
    <div
      className={clsx(
        styles.dropZone,
        state === 'valid' && styles.dropZoneValid,
        state === 'invalid' && styles.dropZoneInvalid,
      )}
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      onMouseLeave={() => setState('idle')}
    >
      {state === 'invalid' ? invalidContent : children}
    </div>
  );
}

type DropZoneMode = 'single' | 'multiple';

export type DropZoneState = 'idle' | 'valid' | 'invalid';

const isFileDrag = (dataTransfer: DataTransfer): boolean =>
  Array.from(dataTransfer.types).includes('Files');

const filesFrom = (dataTransfer: DataTransfer): File[] | null => {
  const files = Array.from(dataTransfer.files);
  return files.length === 0 ? null : files;
};

const areFilesAccepted = (
  files: File[],
  acceptedExtensions: Set<string>,
  mode: DropZoneMode,
): boolean =>
  files.length > 0 &&
  (mode === 'multiple' || files.length === 1) &&
  files.every((file) => {
    const extension = file.name.split('.').pop()?.toLowerCase();
    return extension != null && acceptedExtensions.has(extension);
  });
