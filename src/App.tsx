import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
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

const EMPTY_FORM: FormState = {
  key: "",
  value: "",
  tags: "",
};

function App() {
  const [query, setQuery] = useState("");
  const [entries, setEntries] = useState<EntrySummary[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [editor, setEditor] = useState<EditorState | null>(null);
  const [form, setForm] = useState<FormState>(EMPTY_FORM);
  const [editorLoading, setEditorLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [pasting, setPasting] = useState(false);
  const requestCounter = useRef(0);
  const shouldFocusValueRef = useRef(false);
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

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
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
        void onDeleteSelected();
        return;
      }

      if (event.key === "Escape" && editor) {
        event.preventDefault();
        closeEditor();
        return;
      }

      if (editor) {
        return;
      }

      if (event.key === "Escape") {
        event.preventDefault();
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
        void onPasteSelected();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [editor, editorLoading, entries, query, selectedId]);

  useEffect(() => {
    let blurTimeout: number | null = null;

    const handleWindowBlur = () => {
      if (editor || editorLoading || saving) {
        return;
      }

      blurTimeout = window.setTimeout(() => {
        blurTimeout = null;
        if (document.hasFocus()) {
          return;
        }

        void getCurrentWindow().hide();
      }, 0);
    };

    window.addEventListener("blur", handleWindowBlur);
    return () => {
      if (blurTimeout !== null) {
        window.clearTimeout(blurTimeout);
      }
      window.removeEventListener("blur", handleWindowBlur);
    };
  }, [editor, editorLoading, saving]);

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

  function openCreateModal() {
    const nextKey = entries.length === 0 ? query.trim() : "";

    shouldFocusValueRef.current = true;
    setEditor({ mode: "create" });
    setForm({
      ...EMPTY_FORM,
      key: nextKey,
    });
    setFeedback(null);
  }

  async function openEditModal() {
    if (!selectedEntry || editorLoading) {
      return;
    }

    setEditorLoading(true);
    setFeedback(null);
    try {
      const entry = await getEntry(selectedEntry.id);
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
  }

  function closeEditor() {
    shouldFocusValueRef.current = false;
    setEditor(null);
    setForm(EMPTY_FORM);
    setFeedback(null);
  }

  function moveSelection(direction: 1 | -1) {
    if (entries.length === 0) {
      return;
    }

    const currentIndex = entries.findIndex((entry) => entry.id === selectedId);
    const safeCurrent = currentIndex === -1 ? 0 : currentIndex;
    const nextIndex =
      (safeCurrent + direction + entries.length) % entries.length;
    setSelectedId(entries[nextIndex].id);
  }

  async function onDeleteSelected() {
    if (!selectedEntry) {
      return;
    }

    const approved = window.confirm(
      `Delete "${selectedEntry.key}"? This cannot be undone.`,
    );
    if (!approved) {
      return;
    }

    try {
      await deleteEntry(selectedEntry.id);
      setFeedback(`Deleted "${selectedEntry.key}".`);
      await loadEntries(query);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unable to delete entry";
      setFeedback(message);
    }
  }

  async function onPasteSelected() {
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
  }

  async function onPasteEntry(entryId: string) {
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
  }

  async function onSubmitEditor(event: FormEvent<HTMLFormElement>) {
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
      let successMessage = "";
      if (editor.mode === "create") {
        const created = await createEntry(input);
        setSelectedId(created.id);
        successMessage = `Created "${created.key}".`;
      } else {
        const updated = await updateEntry(editor.id, input);
        setSelectedId(updated.id);
        successMessage = `Updated "${updated.key}".`;
      }
      closeEditor();
      await loadEntries(query);
      setFeedback(successMessage);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : "Unable to save entry";
      setFeedback(message);
    } finally {
      setSaving(false);
    }
  }

  const helperText =
    query.trim().length === 0
      ? "Showing recent snippets"
      : `Search results for "${query}"`;

  const modalTitle = editor?.mode === "create" ? "Add Snippet" : "Edit Snippet";

  const modalSubmitLabel =
    editor?.mode === "create" ? "Create snippet" : "Save changes";

  const modalBody =
    editor?.mode === "edit"
      ? `Editing "${editor.originalKey}"`
      : "Create a new local snippet";

  const isBusy = loading || editorLoading || saving || pasting;

  const formatUpdatedAt = (updatedAt: string) => {
    const parsed = new Date(updatedAt);
    if (Number.isNaN(parsed.getTime())) {
      return updatedAt;
    }
    return parsed.toLocaleString();
  }

  return (
    <main className="launcher">
      <header className="launcherHeader">
        <div className="titleBlock">
          <h1>Snipjar</h1>
          <p>{helperText}</p>
        </div>
        <div className="headerActions">
          <button type="button" onClick={openCreateModal}>
            Add
          </button>
          <button
            type="button"
            onClick={() => {
              void openEditModal();
            }}
            disabled={!selectedEntry || editorLoading}
          >
            Edit
          </button>
          <button
            type="button"
            onClick={() => {
              void onDeleteSelected();
            }}
            disabled={!selectedEntry}
          >
            Delete
          </button>
        </div>
      </header>

      <div className="searchBar">
        <input
          ref={searchInputRef}
          type="text"
          value={query}
          placeholder="Search by key or tags"
          onChange={(event) => setQuery(event.currentTarget.value)}
        />
      </div>

      <section className="results">
        {entries.length === 0 && !isBusy ? (
          <p className="emptyState">No snippets found. Press Cmd+N to add one.</p>
        ) : null}

        <ul>
          {entries.map((entry) => (
            <li key={entry.id}>
              <button
                type="button"
                className={entry.id === selectedId ? "resultRow active" : "resultRow"}
                onClick={() => {
                  setSelectedId(entry.id);
                  void onPasteEntry(entry.id);
                }}
                onMouseEnter={() => setSelectedId(entry.id)}
              >
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
              </button>
            </li>
          ))}
        </ul>
      </section>

      <footer className="statusBar">
        <span>
          Shortcuts: Enter/click paste, Cmd+N add, Cmd+E edit, Cmd+Backspace delete,
          Arrows move selection
        </span>
        {isBusy ? <span>Working...</span> : null}
      </footer>

      {feedback ? <p className="feedback">{feedback}</p> : null}

      {editor ? (
        <div className="modalBackdrop" role="presentation">
          <form className="modal" onSubmit={onSubmitEditor}>
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

            <label htmlFor="entry-value">Value</label>
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
    </main>
  );
}

export default App;
