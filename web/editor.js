import { basicSetup } from "codemirror";
import { indentWithTab } from "@codemirror/commands";
import { LanguageDescription } from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { EditorState } from "@codemirror/state";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView, keymap } from "@codemirror/view";

let view = null;

function cursorPosition(state) {
  const head = state.selection.main.head;
  const line = state.doc.lineAt(head);
  return {
    line: line.number,
    column: head - line.from + 1,
  };
}

// Mount CodeMirror with language detection and editor status callbacks
async function mount(parent, documentText, filename, editable, onChange, onCursor = () => {}) {
  destroy();
  const description = LanguageDescription.matchFilename(languages, filename);
  let language = [];
  if (description) {
    try {
      language = await description.load();
    } catch (error) {
      console.warn(`Could not load the ${description.name} language mode`, error);
    }
  }

  // Configure editor with dark theme, syntax highlighting, and status callbacks
  const state = EditorState.create({
    doc: documentText,
    extensions: [
      basicSetup,
      keymap.of([indentWithTab]),
      oneDark,
      EditorState.readOnly.of(!editable),
      EditorView.editable.of(editable),
      EditorView.lineWrapping,
      EditorView.theme({
        "&": { height: "100%", fontSize: "13px" },
        ".cm-scroller": { overflow: "auto", fontFamily: "ui-monospace, SFMono-Regular, Consolas, monospace" },
      }),
      EditorView.updateListener.of(update => {
        if (update.docChanged) onChange();
        if (update.selectionSet || update.docChanged || update.focusChanged) {
          onCursor(cursorPosition(update.state));
        }
      }),
      language,
    ],
  });

  view = new EditorView({ state, parent });
  view.focus();
  onCursor(cursorPosition(view.state));
}

// Get current document text
function getValue() {
  return view ? view.state.doc.toString() : "";
}

// Move the editor cursor to a 1-based line and column
function focusPosition(lineNumber, columnNumber = 1) {
  if (!view) return false;
  const line = view.state.doc.line(Math.max(1, Math.min(lineNumber, view.state.doc.lines)));
  const column = Math.max(1, Math.min(columnNumber, line.length + 1));
  const position = line.from + column - 1;
  view.dispatch({
    selection: { anchor: position },
    scrollIntoView: true,
  });
  view.focus();
  return true;
}

// Destroy editor instance
function destroy() {
  if (view) {
    view.destroy();
    view = null;
  }
}

// Export API for host
window.LiveEditor = { mount, getValue, focusPosition, destroy };
