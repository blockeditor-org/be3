package com.be3.beui;

import android.text.Editable;
import android.text.Selection;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.TextAttribute;

final class BeuiInputConnection extends BaseInputConnection {
    private static final int SET_COMPOSING_TEXT = 0;
    private static final int COMMIT_TEXT = 1;
    private static final int FINISH_COMPOSING = 2;
    private static final int SET_COMPOSING_REGION = 3;
    private static final int REPLACE_TEXT = 4;
    private static final int DELETE_SURROUNDING = 5;
    private static final int SET_SELECTION = 6;

    private final BeuiView view;
    private long start;
    private int batch;
    private boolean closed;

    BeuiInputConnection(BeuiView view, BeuiView.ImeText state) {
        super(view, true);
        this.view = view;
        load(state);
    }

    void close() {
        closed = true;
    }

    void sync(BeuiView.ImeText state) {
        if (closed || batch > 0) return;
        Editable text = getEditable();
        int selectionStart = state.index(state.selectionStart);
        int selectionEnd = state.index(state.selectionEnd);
        int composingStart = state.composingStart < 0 ? -1 : state.index(state.composingStart);
        int composingEnd = state.composingEnd < 0 ? -1 : state.index(state.composingEnd);
        boolean same = start == state.start
                && text.toString().equals(state.text)
                && Selection.getSelectionStart(text) == selectionStart
                && Selection.getSelectionEnd(text) == selectionEnd
                && composingStart() == composingStart
                && composingEnd() == composingEnd;
        if (same) return;
        load(state);
        report();
    }

    private void load(BeuiView.ImeText state) {
        Editable text = getEditable();
        start = state.start;
        if (!text.toString().equals(state.text)) text.replace(0, text.length(), state.text);
        Selection.setSelection(text, state.index(state.selectionStart),
                state.index(state.selectionEnd));
        if (state.composingStart >= 0 && state.composingEnd > state.composingStart) {
            super.setComposingRegion(state.index(state.composingStart),
                    state.index(state.composingEnd));
        } else {
            removeComposingSpans(text);
        }
    }

    @Override
    public boolean beginBatchEdit() {
        batch++;
        return true;
    }

    @Override
    public boolean endBatchEdit() {
        if (batch > 0) batch--;
        if (batch == 0) {
            report();
            view.syncIme();
        }
        return batch > 0;
    }

    @Override
    public boolean setComposingText(CharSequence text, int position) {
        if (closed) return false;
        int from = composingStart() >= 0 ? composingStart() : low(getEditable());
        send(SET_COMPOSING_TEXT, 0, 0, text.toString());
        super.setComposingText(text, position);
        follow(from + text.length());
        changed();
        return true;
    }

    @Override
    public boolean commitText(CharSequence text, int position) {
        if (closed) return false;
        int from = composingStart() >= 0 ? composingStart() : low(getEditable());
        send(COMMIT_TEXT, 0, 0, text.toString());
        super.commitText(text, position);
        follow(from + text.length());
        changed();
        return true;
    }

    @Override
    public boolean finishComposingText() {
        if (closed) return false;
        if (composingStart() >= 0) send(FINISH_COMPOSING, 0, 0, null);
        super.finishComposingText();
        changed();
        return true;
    }

    @Override
    public boolean setComposingRegion(int start, int end) {
        if (closed) return false;
        Editable text = getEditable();
        int from = clamp(Math.min(start, end), text);
        int to = clamp(Math.max(start, end), text);
        send(SET_COMPOSING_REGION, offset(from), offset(to), null);
        super.setComposingRegion(from, to);
        changed();
        return true;
    }

    @Override
    public boolean replaceText(int start, int end, CharSequence text, int position,
            TextAttribute attribute) {
        if (closed) return false;
        Editable content = getEditable();
        int from = clamp(Math.min(start, end), content);
        int to = clamp(Math.max(start, end), content);
        beginBatchEdit();
        send(REPLACE_TEXT, offset(from), offset(to), text.toString());
        super.replaceText(from, to, text, position, attribute);
        follow(from + text.length());
        endBatchEdit();
        return true;
    }

    @Override
    public boolean deleteSurroundingText(int before, int after) {
        if (closed) return false;
        Editable text = getEditable();
        int low = low(text);
        int high = high(text);
        int from = Math.max(0, low - Math.max(0, before));
        int to = Math.min(text.length(), high + Math.max(0, after));
        if (from > 0 && Character.isLowSurrogate(text.charAt(from))) from--;
        if (to < text.length() && Character.isLowSurrogate(text.charAt(to))) to++;
        if (from < low || high < to) {
            send(DELETE_SURROUNDING, utf8(text, from, low), utf8(text, high, to), null);
        }
        super.deleteSurroundingText(low - from, to - high);
        changed();
        return true;
    }

    @Override
    public boolean deleteSurroundingTextInCodePoints(int before, int after) {
        if (closed) return false;
        Editable text = getEditable();
        int low = low(text);
        int high = high(text);
        int backward = Math.min(Math.max(0, before), Character.codePointCount(text, 0, low));
        int forward = Math.min(Math.max(0, after),
                Character.codePointCount(text, high, text.length()));
        int from = Character.offsetByCodePoints(text, low, -backward);
        int to = Character.offsetByCodePoints(text, high, forward);
        return deleteSurroundingText(low - from, to - high);
    }

    @Override
    public boolean setSelection(int start, int end) {
        if (closed) return false;
        Editable text = getEditable();
        if (start < 0 || end < 0 || start > text.length() || end > text.length()) return true;
        send(SET_SELECTION, offset(start), offset(end), null);
        super.setSelection(start, end);
        changed();
        return true;
    }

    private void follow(int expected) {
        Editable text = getEditable();
        int anchor = Selection.getSelectionStart(text);
        int focus = Selection.getSelectionEnd(text);
        expected = clamp(expected, text);
        if (anchor < 0 || focus < 0 || (anchor == expected && focus == expected)) return;
        send(SET_SELECTION, offset(anchor), offset(focus), null);
    }

    private void send(int kind, long first, long second, String text) {
        BeuiView.nativeIme(kind, first, second, text, view.nextImeSerial());
    }

    private long offset(int index) {
        return start + utf8(getEditable(), 0, index);
    }

    private void changed() {
        if (batch == 0) report();
    }

    private void report() {
        Editable text = getEditable();
        view.updateSelection(Selection.getSelectionStart(text), Selection.getSelectionEnd(text),
                composingStart(), composingEnd());
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

    private static int low(Editable text) {
        return clamp(Math.min(Selection.getSelectionStart(text), Selection.getSelectionEnd(text)),
                text);
    }

    private static int high(Editable text) {
        return clamp(Math.max(Selection.getSelectionStart(text), Selection.getSelectionEnd(text)),
                text);
    }

    private static int clamp(int index, Editable text) {
        return Math.max(0, Math.min(index, text.length()));
    }

    static long utf8(CharSequence text, int from, int to) {
        long bytes = 0;
        for (int index = from; index < to; index++) {
            char letter = text.charAt(index);
            if (letter < 0x80) {
                bytes += 1;
            } else if (letter < 0x800) {
                bytes += 2;
            } else if (Character.isHighSurrogate(letter) && index + 1 < to
                    && Character.isLowSurrogate(text.charAt(index + 1))) {
                bytes += 4;
                index++;
            } else {
                bytes += 3;
            }
        }
        return bytes;
    }
}
