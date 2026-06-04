import clsx from 'clsx';
import { useMemo, useRef, useState, type ReactNode } from 'react';
import { getKeySnapshot } from '../../utils/browser';
import styles from './DropZone.module.scss';

export type DropZoneMode = 'single' | 'multiple';

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

  const submitFiles = (files: File[] | null) => {
    if (!files || !areFilesAccepted(files, acceptedExtensions, mode)) {
      setState('invalid');
      return;
    }

    setState('idle');
    onDropFiles(files);
  };
  const openFileDialog = useFileDialog({
    extensions,
    mode,
    onFilesSelected: submitFiles,
  });

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
    }

    submitFiles(files);
  };

  const onKeyDown = (event: KeyboardEvent) => {
    const key = getKeySnapshot(event);
    if (key !== 'enter' && key !== 'space') {
      return;
    }

    event.preventDefault();
    openFileDialog();
  };

  return (
    <div
      className={clsx(
        styles.dropZone,
        state === 'valid' && styles.dropZoneValid,
        state === 'invalid' && styles.dropZoneInvalid,
      )}
      role="button"
      tabIndex={0}
      onClick={openFileDialog}
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
      onKeyDown={onKeyDown}
      onMouseLeave={() => setState('idle')}
    >
      {state === 'invalid' ? invalidContent : children}
    </div>
  );
}

function useFileDialog({
  extensions,
  mode,
  onFilesSelected,
}: {
  extensions: string[];
  mode: DropZoneMode;
  onFilesSelected: (files: File[] | null) => void;
}): () => void {
  const accept = useMemo(
    () =>
      extensions
        .map((ext) => `.${ext.replace(/^\./, '').toLowerCase()}`)
        .join(','),
    [extensions],
  );

  return () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = accept;
    input.multiple = mode === 'multiple';
    input.addEventListener(
      'change',
      () => {
        const files = Array.from(input.files ?? []);
        onFilesSelected(files.length === 0 ? null : files);
        input.value = '';
      },
      { once: true },
    );
    input.click();
  };
}

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
