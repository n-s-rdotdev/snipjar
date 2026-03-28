import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import {
  createEntry,
  deleteEntry,
  getEntry,
  getRecentEntries,
  pasteEntry,
  searchEntries,
  updateEntry,
} from "./api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { EntryInput, EntrySummary } from "./types";
import "./App.css";

type EditorState =
  | { mode: "create" }
  | { mode: "edit"; id: string; originalKey: string };

type FormState = {
  key: string;
  value: string;
  tags: string;
};

type DeleteState = {
  id: string;
  key: string;
};

const EMPTY_FORM: FormState = {
  key: "",
  value: "",
  tags: "",
};

function App() {
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<EntrySummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [expandedActionId, setExpandedActionId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [deleteState, setDeleteState] = useState<DeleteState | null>(null);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [editorLoading, setEditorLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [pasting, setPasting] = useState(false);
  const requestCounter = useRef(0);
  const shouldFocusValueRef = useRef(false);
  const suppressBlurUntilRef = useRef(0);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const valueInputRef = useRef<HTMLTextAreaElement | null>(null);

  const selectedEntry = useMemo(() => {
    if (!selectedId) {
      return null;
    }
    return entries.find((entry) => entry.id === selectedId) ?? null;
  }, [entries, selectedId]);

  useEffect(() => {
    if (entries.length === 0) {
      setSelectedId(null);
      return;
    }

    if (!selectedId || !entries.some((entry) => entry.id === selectedId)) {
      setSelectedId(entries[0].id);
    }
  }, [entries, selectedId]);

  useEffect(() => {
    if (!expandedActionId) {
      return;
    }

    if (!entries.some((entry) => entry.id === expandedActionId)) {
      setExpandedActionId(null);
    }
  }, [entries, expandedActionId]);

  useEffect(() => {
    if (!expandedActionId) {
      return;
    }

    if (selectedId !== expandedActionId) {
      setExpandedActionId(null);
    }
  }, [selectedId, expandedActionId]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void loadEntries(query);
    }, 120);

    return () => {
      window.clearTimeout(timeout);
    };
  }, [query]);

  useEffect(() => {
    if (editor) {
      return;
    }

    const frame = window.requestAnimationFrame(() => {
      searchInputRef.current?.focus();
    });

    return () => {
      window.cancelAnimationFrame(frame);
    };
  }, [editor]);

  useLayoutEffect(() => {
    if (!editor || editorLoading || !shouldFocusValueRef.current) {
      return;
    }

    shouldFocusValueRef.current = false;
    const node = valueInputRef.current;
    if (!node) {
      return;
    }

    node.focus();
    const length = node.value.length;
    node.setSelectionRange(length, length);
  }, [editor, editorLoading]);

  async function loadEntries(nextQuery: string) {
    requestCounter.current += 1;
    const currentRequest = requestCounter.current;

    setLoading(true);
    try {
      const nextEntries =
        nextQuery.trim().length === 0
          ? await getRecentEntries()
          : await searchEntries(nextQuery);
      if (requestCounter.current !== currentRequest) {
        return;
      }
      setEntries(nextEntries);
      setFeedback(null);
    } catch (error) {
      if (requestCounter.current !== currentRequest) {
        return;
      }
      const message =
        error instanceof Error ? error.message : "Unable to load entries";
      setFeedback(message);
    } finally {
      if (requestCounter.current === currentRequest) {
        setLoading(false);
      }
    }
  }

  const openCreateModal = useCallback(() => {
    const nextKey = entries.length === 0 ? query.trim() : "";

    shouldFocusValueRef.current = true;
    setEditor({ mode: "create" });
    setForm({
      ...EMPTY_FORM,
      key: nextKey,
    });
    setFeedback(null);
  }, [entries, query]);

  const openEditModal = useCallback(async (overrideId?: string) => {
    const targetId = overrideId ?? selectedId;
    const targetEntry = entries.find((e) => e.id === targetId) ?? null;

    if (!targetEntry || editorLoading) {
      return;
    }

    setEditorLoading(true);
    setFeedback(null);
    try {
      const entry = await getEntry(targetEntry.id);
      shouldFocusValueRef.current = true;
      setEditor({
        mode: "edit",
        id: entry.id,
        originalKey: entry.key,
      });
      setForm({
        key: entry.key,
        value: entry.value,
        tags: entry.tags.join(", "),
      });
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unable to load entry";
      setFeedback(message);
    } finally {
      setEditorLoading(false);
    }
  }, [entries, selectedId, editorLoading]);

  const closeEditor = useCallback(() => {
    shouldFocusValueRef.current = false;
    setEditor(null);
    setForm(EMPTY_FORM);
    setFeedback(null);
  }, []);

  const closeDeleteDialog = useCallback(() => {
    setDeleteState(null);
    setFeedback(null);
  }, []);

  const moveSelection = useCallback((direction: 1 | -1) => {
    if (entries.length === 0) {
      return;
    }

    const currentIndex = entries.findIndex((entry) => entry.id === selectedId);
    const safeCurrent = currentIndex === -1 ? 0 : currentIndex;
    const nextIndex =
      (safeCurrent + direction + entries.length) % entries.length;
    setSelectedId(entries[nextIndex].id);
  }, [entries, selectedId]);

  const toggleEntryActions = useCallback((entryId: string) => {
    setSelectedId(entryId);
    setExpandedActionId((currentId) =>
      currentId === entryId ? null : entryId,
    );
  }, []);

  const onDeleteSelected = useCallback((overrideId?: string) => {
    const targetId = overrideId ?? selectedId;
    const targetEntry = entries.find((e) => e.id === targetId) ?? null;

    if (!targetEntry) {
      return;
    }

    setSelectedId(targetEntry.id);
    setDeleteState({
      id: targetEntry.id,
      key: targetEntry.key,
    });
    setFeedback(null);
  }, [selectedId, entries]);

  const confirmDelete = useCallback(async () => {
    if (!deleteState) {
      return;
    }

    setDeleting(true);
    try {
      await deleteEntry(deleteState.id);
      setDeleteState(null);
      setFeedback(`Deleted "${deleteState.key}".`);
      await loadEntries(query);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unable to delete entry";
      setFeedback(message);
    } finally {
      setDeleting(false);
    }
  }, [deleteState, query]);

  const onPasteSelected = useCallback(async () => {
    if (!selectedEntry) {
      return;
    }

    setPasting(true);
    try {
      const result = await pasteEntry(selectedEntry.id);
      setFeedback(result.mode === "copied_only" ? result.message : null);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unable to paste selected entry";
      setFeedback(message);
    } finally {
      setPasting(false);
    }
  }, [selectedEntry]);

  const onPasteEntry = useCallback(async (entryId: string) => {
    setPasting(true);
    try {
      const result = await pasteEntry(entryId);
      setFeedback(result.mode === "copied_only" ? result.message : null);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unable to paste selected entry";
      setFeedback(message);
    } finally {
      setPasting(false);
    }
  }, []);

  const onSubmitEditor = useCallback(async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (!editor) {
      return;
    }

    const input: EntryInput = {
      key: form.key,
      value: form.value,
      tags: form.tags
        .split(",")
        .map((tag) => tag.trim())
        .filter((tag) => tag.length > 0),
    };

    setSaving(true);
    try {
      if (editor.mode === "create") {
        const created = await createEntry(input);
        setSelectedId(created.id);
      } else {
        const updated = await updateEntry(editor.id, input);
        setSelectedId(updated.id);
      }
      closeEditor();
      await loadEntries(query);
      setFeedback(null);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unable to save entry";
      setFeedback(message);
    } finally {
      setSaving(false);
    }
  }, [editor, form, query, closeEditor]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (deleteState) {
        if (event.key === "Escape") {
          event.preventDefault();
          closeDeleteDialog();
          return;
        }

        if (event.key === "Enter") {
          event.preventDefault();
          void confirmDelete();
        }

        return;
      }

      if (editor) {
        if (event.key === "Escape") {
          event.preventDefault();
          closeEditor();
        }

        return;
      }

      if (event.metaKey && event.key.toLowerCase() === "n") {
        event.preventDefault();
        openCreateModal();
        return;
      }

      if (event.metaKey && event.key.toLowerCase() === "e") {
        event.preventDefault();
        void openEditModal();
        return;
      }

      if (event.metaKey && event.key === "Backspace") {
        event.preventDefault();
        onDeleteSelected();
        return;
      }

      if (event.key === "Escape" && expandedActionId) {
        event.preventDefault();
        setExpandedActionId(null);
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
        setQuery("");
        void getCurrentWindow().hide();
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        moveSelection(1);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        moveSelection(-1);
      } else if (event.key === "Enter") {
        event.preventDefault();
        if (selectedId) {
          void onPasteSelected();
        } else if (query.trim().length > 0) {
          openCreateModal();
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [deleteState, editor, query, selectedId, expandedActionId, openCreateModal, openEditModal, onDeleteSelected, closeDeleteDialog, confirmDelete, closeEditor, moveSelection, onPasteSelected]);

  useEffect(() => {
    const currentWindow = getCurrentWindow();
    let unlisten: (() => void) | null = null;
    let disposed = false;

    void currentWindow
      .onFocusChanged(({ payload: focused }) => {
        if (focused) {
          suppressBlurUntilRef.current = Date.now() + 150;
          return;
        }

        if (Date.now() < suppressBlurUntilRef.current) {
          return;
        }

        setQuery("");
        closeEditor();
        closeDeleteDialog();
        void currentWindow.hide();
      })
      .then((nextUnlisten) => {
        if (disposed) {
          nextUnlisten();
          return;
        }

        unlisten = nextUnlisten;
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [closeDeleteDialog, closeEditor]);

  const modalTitle = editor?.mode === "create" ? "Add Snippet" : "Edit Snippet";

  const modalSubmitLabel =
    editor?.mode === "create" ? "Create snippet" : "Save changes";

  const modalBody =
    editor?.mode === "edit"
      ? `Editing "${editor.originalKey}"`
      : "Create a new local snippet";

  const isBusy = loading || editorLoading || saving || deleting || pasting;

  const formatUpdatedAt = (updatedAt: string) => {
    const parsed = new Date(updatedAt);
    if (Number.isNaN(parsed.getTime())) {
      return updatedAt;
    }
    return parsed.toLocaleString();
  }

  return (
    <main className="launcher">
      <div className="searchBar">
        <input
          ref={searchInputRef}
          type="text"
          value={query}
          placeholder="Search by key or tags..."
          onChange={(event) => setQuery(event.currentTarget.value)}
        />
      </div>

      <section className="results">
        {entries.length === 0 && !isBusy ? (
          <p className="emptyState">No snippets found. Press Cmd+N to add one.</p>
        ) : null}

        <ul>
          {entries.map((entry) => {
            const isActive = entry.id === selectedId;
            const isExpanded = entry.id === expandedActionId;

            return (
              <li
                key={entry.id}
                className={[
                  "resultItem",
                  isActive ? "active" : "",
                  isExpanded ? "expanded" : "",
                ].filter(Boolean).join(" ")}
              >
                <div
                  className={isActive ? "resultRow active" : "resultRow"}
                  onClick={() => {
                    setSelectedId(entry.id);
                    setExpandedActionId(null);
                    void onPasteEntry(entry.id);
                  }}
                  onMouseEnter={() => setSelectedId(entry.id)}
                >
                  <div className="rowContent">
                    <div className="rowTop">
                      <strong>{entry.key}</strong>
                      <span>{formatUpdatedAt(entry.updatedAt)}</span>
                    </div>
                    <div className="tags">
                      {entry.tags.length === 0 ? (
                        <em>No tags</em>
                      ) : (
                        entry.tags.map((tag) => (
                          <span key={`${entry.id}-${tag}`} className="tagPill">
                            {tag}
                          </span>
                        ))
                      )}
                    </div>
                  </div>
                  <button
                    type="button"
                    className={isExpanded ? "chevronBtn expanded" : "chevronBtn"}
                    onClick={(event) => {
                      event.stopPropagation();
                      toggleEntryActions(entry.id);
                    }}
                    aria-expanded={isExpanded}
                    aria-label={isExpanded ? `Hide actions for ${entry.key}` : `Show actions for ${entry.key}`}
                    title={isExpanded ? "Hide actions" : "Show actions"}
                  >
                    <svg viewBox="0 0 16 16" aria-hidden="true">
                      <path
                        d="M4 6.5 8 10.5l4-4"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="1.75"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  </button>
                </div>
                {isExpanded ? (
                  <div className="expandedActions" onClick={(event) => event.stopPropagation()}>
                    <button
                      type="button"
                      className="actionBtn expandedActionBtn"
                      onClick={() => {
                        setExpandedActionId(null);
                        void openEditModal(entry.id);
                      }}
                      title="Edit (Cmd+E)"
                    >
                      Edit
                    </button>
                    <button
                      type="button"
                      className="actionBtn expandedActionBtn deleteBtn"
                      onClick={() => {
                        setExpandedActionId(null);
                        void onDeleteSelected(entry.id);
                      }}
                      title="Delete (Cmd+Backspace)"
                    >
                      Delete
                    </button>
                  </div>
                ) : null}
              </li>
            );
          })}
        </ul>
      </section>

      <footer className="statusBar">
        <div className="statusLeft">
          <div className="logoLockup">
            <img className="logoMark" src="/snipjar-icon.svg" alt="Snipjar" />
            <span className="logoLabel">Snipjar</span>
          </div>
        </div>
        <div className="statusRight">
          {isBusy ? (
            <span>Working...</span>
          ) : (
            <div className="shortcuts">
              <span>Shortcuts:</span>
              <span>⏎ Paste</span>
              <span>⌘N Add</span>
              <span>⌘E Edit</span>
              <span>⌘⌫ Delete</span>
              <span>↓↑ Navigate</span>
            </div>
          )}
        </div>
      </footer>

      {feedback ? <p className="feedback">{feedback}</p> : null}

      {editor ? (
        <div className="modalBackdrop" role="presentation" onClick={closeEditor}>
          <form className="modal" onSubmit={onSubmitEditor} onClick={(event) => event.stopPropagation()}>
            <h2>{modalTitle}</h2>
            <p>{modalBody}</p>

            <label htmlFor="entry-key">Key</label>
            <input
              id="entry-key"
              value={form.key}
              onChange={(event) => {
                const nextValue = event.currentTarget.value;
                setForm((current) => ({ ...current, key: nextValue }));
              }}
              placeholder="example: work-email"
              required
            />

            <div className="labelRow">
              <label htmlFor="entry-value">Value</label>
              <span className="tip">Shift + Enter for new line</span>
            </div>
            <textarea
              id="entry-value"
              ref={valueInputRef}
              value={form.value}
              onChange={(event) => {
                const nextValue = event.currentTarget.value;
                setForm((current) => ({
                  ...current,
                  value: nextValue,
                }));
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !event.shiftKey) {
                  event.preventDefault();
                  event.currentTarget.form?.requestSubmit();
                }
              }}
              placeholder="Snippet text to paste later"
              required
            />

            <label htmlFor="entry-tags">Tags</label>
            <input
              id="entry-tags"
              value={form.tags}
              onChange={(event) => {
                const nextValue = event.currentTarget.value;
                setForm((current) => ({ ...current, tags: nextValue }));
              }}
              placeholder="comma,separated,tags"
            />

            <div className="modalActions">
              <button type="button" onClick={closeEditor} disabled={saving}>
                Cancel
              </button>
              <button type="submit" disabled={saving}>
                {modalSubmitLabel}
              </button>
            </div>
          </form>
        </div>
      ) : null}

      {deleteState ? (
        <div className="modalBackdrop" role="presentation" onClick={closeDeleteDialog}>
          <div
            className="modal"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="delete-snippet-title"
            aria-describedby="delete-snippet-description"
            onClick={(event) => event.stopPropagation()}
          >
            <h2 id="delete-snippet-title">Delete Snippet</h2>
            <p id="delete-snippet-description">
              {`Delete "${deleteState.key}"? This cannot be undone.`}
            </p>

            <div className="modalActions">
              <button type="button" onClick={closeDeleteDialog} disabled={deleting}>
                Cancel
              </button>
              <button
                type="button"
                className="dangerButton"
                onClick={() => {
                  void confirmDelete();
                }}
                disabled={deleting}
              >
                {deleting ? "Deleting..." : "Delete snippet"}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </main>
  );
}

export default App;
