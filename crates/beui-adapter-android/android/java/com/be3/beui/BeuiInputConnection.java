package com.be3.beui;

import android.text.Editable;
import android.text.Selection;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.TextAttribute;

final class BeuiInputConnection extends BaseInputConnection {
    private static final int TRIM_AFTER = 1024;
    private static final int KEEP = 256;

    private final BeuiView view;
    private int batch;
    private boolean closed;

    BeuiInputConnection(BeuiView view) {
        super(view, true);
        this.view = view;
    }

    void close() {
        closed = true;
    }

    @Override
    public boolean beginBatchEdit() {
        batch++;
        return true;
    }

    @Override
    public boolean endBatchEdit() {
        if (batch > 0) batch--;
        if (batch == 0) report();
        return batch > 0;
    }

    @Override
    public boolean setComposingText(CharSequence text, int position) {
        if (closed) return false;
        super.setComposingText(text, position);
        BeuiView.nativeCompose(text.toString());
        changed();
        return true;
    }

    @Override
    public boolean commitText(CharSequence text, int position) {
        if (closed) return false;
        int start = composingStart();
        boolean composing = start >= 0;
        if (!composing) start = selectionStart(getEditable());
        super.commitText(text, position);
        BeuiView.nativeCommit(text.toString(), composing);
        follow(start + text.length());
        changed();
        return true;
    }

    @Override
    public boolean replaceText(int start, int end, CharSequence text, int position,
            TextAttribute attribute) {
        if (closed) return false;
        beginBatchEdit();
        settle();
        Editable content = getEditable();
        int from = clamp(Math.min(start, end), content);
        int to = clamp(Math.max(start, end), content);
        int cursor = Selection.getSelectionEnd(content);
        if (cursor >= 0) BeuiView.nativeMove(codePoints(content, cursor, to));
        int removed = Character.codePointCount(content, from, to);
        if (removed > 0) BeuiView.nativeDelete(removed, 0);
        super.replaceText(start, end, text, position, attribute);
        if (text.length() > 0) BeuiView.nativeCommit(text.toString(), false);
        follow(from + text.length());
        endBatchEdit();
        return true;
    }

    private void follow(int at) {
        Editable text = getEditable();
        int cursor = Selection.getSelectionEnd(text);
        if (cursor < 0 || cursor != Selection.getSelectionStart(text)) return;
        int move = codePoints(text, clamp(at, text), cursor);
        if (move != 0) BeuiView.nativeMove(move);
    }

    private static int codePoints(Editable text, int from, int to) {
        return from <= to
                ? Character.codePointCount(text, from, to)
                : -Character.codePointCount(text, to, from);
    }

    @Override
    public boolean finishComposingText() {
        if (closed) return false;
        settle();
        changed();
        return true;
    }

    private void settle() {
        String composed = composed();
        super.finishComposingText();
        if (composed != null) BeuiView.nativeCommit(composed, true);
    }

    @Override
    public boolean setComposingRegion(int start, int end) {
        if (closed) return false;
        settle();
        Editable text = getEditable();
        int from = clamp(Math.min(start, end), text);
        int to = clamp(Math.max(start, end), text);
        int cursor = Selection.getSelectionEnd(text);
        super.setComposingRegion(start, end);
        if (from < to && cursor >= 0) {
            int move = cursor <= to
                    ? Character.codePointCount(text, cursor, to)
                    : -Character.codePointCount(text, to, cursor);
            BeuiView.nativeRecompose(move, Character.codePointCount(text, from, to),
                    text.subSequence(from, to).toString());
        }
        changed();
        return true;
    }

    @Override
    public boolean deleteSurroundingText(int before, int after) {
        if (closed) return false;
        settle();
        Editable text = getEditable();
        int start = selectionStart(text);
        int end = selectionEnd(text);
        int from = Math.max(0, start - Math.max(0, before));
        int to = Math.min(text.length(), end + Math.max(0, after));
        int backward = Character.codePointCount(text, from, start);
        int forward = Character.codePointCount(text, end, to);
        super.deleteSurroundingText(before, after);
        if (backward > 0 || forward > 0) BeuiView.nativeDelete(backward, forward);
        changed();
        return true;
    }

    @Override
    public boolean deleteSurroundingTextInCodePoints(int before, int after) {
        if (closed) return false;
        settle();
        Editable text = getEditable();
        int start = selectionStart(text);
        int end = selectionEnd(text);
        int backward = Math.min(Math.max(0, before), Character.codePointCount(text, 0, start));
        int forward = Math.min(Math.max(0, after),
                Character.codePointCount(text, end, text.length()));
        int from = Character.offsetByCodePoints(text, start, -backward);
        int to = Character.offsetByCodePoints(text, end, forward);
        return deleteSurroundingText(start - from, to - end);
    }

    @Override
    public boolean setSelection(int start, int end) {
        if (closed) return false;
        settle();
        Editable text = getEditable();
        int cursor = Selection.getSelectionEnd(text);
        boolean collapsed = start == end && start >= 0 && start <= text.length();
        int move = 0;
        if (collapsed && cursor >= 0) {
            move = start >= cursor
                    ? Character.codePointCount(text, cursor, start)
                    : -Character.codePointCount(text, start, cursor);
        }
        super.setSelection(start, end);
        if (move != 0) BeuiView.nativeMove(move);
        changed();
        return true;
    }

    private void changed() {
        if (batch == 0) report();
    }

    private void report() {
        Editable text = getEditable();
        if (composingStart() < 0 && text.length() > TRIM_AFTER) {
            int cut = Math.min(text.length() - KEEP, selectionStart(text));
            if (cut > 0 && cut < text.length() && Character.isLowSurrogate(text.charAt(cut))) {
                cut++;
            }
            if (cut > 0) text.delete(0, cut);
        }
        view.updateSelection(Selection.getSelectionStart(text), Selection.getSelectionEnd(text),
                composingStart(), composingEnd());
    }

    private String composed() {
        int start = composingStart();
        int end = composingEnd();
        if (start < 0) return null;
        return getEditable().subSequence(start, end).toString();
    }

    private int composingStart() {
        Editable text = getEditable();
        int start = getComposingSpanStart(text);
        int end = getComposingSpanEnd(text);
        return start < 0 || end < 0 ? -1 : Math.min(start, end);
    }

    private int composingEnd() {
        Editable text = getEditable();
        int start = getComposingSpanStart(text);
        int end = getComposingSpanEnd(text);
        return start < 0 || end < 0 ? -1 : Math.max(start, end);
    }

    private static int selectionStart(Editable text) {
        return clamp(Math.min(Selection.getSelectionStart(text), Selection.getSelectionEnd(text)),
                text);
    }

    private static int selectionEnd(Editable text) {
        return clamp(Math.max(Selection.getSelectionStart(text), Selection.getSelectionEnd(text)),
                text);
    }

    private static int clamp(int index, Editable text) {
        return Math.max(0, Math.min(index, text.length()));
    }
}
