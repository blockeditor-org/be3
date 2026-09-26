package com.be3.beui;

import android.app.Activity;
import android.content.Context;
import android.content.res.AssetManager;
import android.graphics.Insets;
import android.os.Build;
import android.view.DisplayCutout;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.WindowInsets;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;

public final class BeuiView extends SurfaceView implements SurfaceHolder.Callback {
    private static final int TOUCH_START = 0;
    private static final int TOUCH_MOVE = 1;
    private static final int TOUCH_END = 2;
    private static final int TOUCH_CANCEL = 3;

    private final InputMethodManager input;
    private boolean keyboard;
    private BeuiInputConnection connection;

    BeuiView(Context context) {
        super(context);
        input = (InputMethodManager) context.getSystemService(Context.INPUT_METHOD_SERVICE);
        getHolder().addCallback(this);
        setFocusable(true);
        setFocusableInTouchMode(true);
    }

    @Override
    public void surfaceCreated(SurfaceHolder holder) {}

    @Override
    public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        nativeSurfaceChanged(holder.getSurface(), width, height,
                getResources().getDisplayMetrics().density);
    }

    @Override
    public void surfaceDestroyed(SurfaceHolder holder) {
        nativeSurfaceDestroyed();
    }

    @Override
    public WindowInsets onApplyWindowInsets(WindowInsets insets) {
        if (Build.VERSION.SDK_INT >= 30) {
            Insets safe = insets.getInsets(
                    WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            Insets ime = insets.getInsets(WindowInsets.Type.ime());
            nativeInsets(safe.left, safe.top, safe.right, Math.max(safe.bottom, ime.bottom));
            return insets;
        }
        int left = insets.getSystemWindowInsetLeft();
        int top = insets.getSystemWindowInsetTop();
        int right = insets.getSystemWindowInsetRight();
        int bottom = insets.getSystemWindowInsetBottom();
        if (Build.VERSION.SDK_INT >= 28) {
            DisplayCutout cutout = insets.getDisplayCutout();
            if (cutout != null) {
                left = Math.max(left, cutout.getSafeInsetLeft());
                top = Math.max(top, cutout.getSafeInsetTop());
                right = Math.max(right, cutout.getSafeInsetRight());
                bottom = Math.max(bottom, cutout.getSafeInsetBottom());
            }
        }
        nativeInsets(left, top, right, bottom);
        return insets;
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
            case MotionEvent.ACTION_POINTER_DOWN:
                touch(event, event.getActionIndex(), TOUCH_START);
                return true;
            case MotionEvent.ACTION_MOVE:
                for (int index = 0; index < event.getPointerCount(); index++) {
                    touch(event, index, TOUCH_MOVE);
                }
                return true;
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_POINTER_UP:
                touch(event, event.getActionIndex(), TOUCH_END);
                return true;
            case MotionEvent.ACTION_CANCEL:
                for (int index = 0; index < event.getPointerCount(); index++) {
                    touch(event, index, TOUCH_CANCEL);
                }
                return true;
            default:
                return false;
        }
    }

    private static void touch(MotionEvent event, int index, int phase) {
        nativeTouch(event.getDeviceId(), event.getPointerId(index), phase, event.getX(index),
                event.getY(index), event.getPressure(index));
    }

    @Override
    public boolean onGenericMotionEvent(MotionEvent event) {
        if (event.getActionMasked() == MotionEvent.ACTION_SCROLL) {
            nativeScroll(-event.getAxisValue(MotionEvent.AXIS_HSCROLL),
                    event.getAxisValue(MotionEvent.AXIS_VSCROLL));
            return true;
        }
        return super.onGenericMotionEvent(event);
    }

    @Override
    public boolean onKeyDown(int code, KeyEvent event) {
        return key(event, true) || super.onKeyDown(code, event);
    }

    @Override
    public boolean onKeyUp(int code, KeyEvent event) {
        return key(event, false) || super.onKeyUp(code, event);
    }

    private static boolean key(KeyEvent event, boolean pressed) {
        int meta = event.getMetaState();
        return nativeKey(event.getKeyCode(), pressed, event.getRepeatCount(), meta,
                event.getUnicodeChar(meta));
    }

    @Override
    public boolean onCheckIsTextEditor() {
        return keyboard;
    }

    @Override
    public InputConnection onCreateInputConnection(EditorInfo info) {
        if (!keyboard) return null;
        info.inputType = EditorInfo.TYPE_CLASS_TEXT
                | EditorInfo.TYPE_TEXT_FLAG_MULTI_LINE
                | EditorInfo.TYPE_TEXT_FLAG_AUTO_CORRECT;
        info.imeOptions = EditorInfo.IME_FLAG_NO_FULLSCREEN
                | EditorInfo.IME_FLAG_NO_EXTRACT_UI
                | EditorInfo.IME_ACTION_NONE;
        info.initialSelStart = 0;
        info.initialSelEnd = 0;
        connection = new BeuiInputConnection(this);
        return connection;
    }

    void updateSelection(int start, int end, int composingStart, int composingEnd) {
        input.updateSelection(this, start, end, composingStart, composingEnd);
    }

    public void setKeyboard(boolean shown) {
        post(() -> {
            if (connection != null) {
                connection.close();
                connection = null;
            }
            if (shown) {
                keyboard = true;
                requestFocus();
                input.restartInput(this);
                input.showSoftInput(this, 0);
            } else if (keyboard) {
                keyboard = false;
                input.restartInput(this);
                input.hideSoftInputFromWindow(getWindowToken(), 0);
            }
        });
    }

    public void finishActivity() {
        post(() -> ((Activity) getContext()).finish());
    }

    static native void nativeCreate(Activity activity, BeuiView view, AssetManager assets,
            String files);

    static native void nativeDestroy(boolean finishing);

    static native void nativeFocus(boolean focused);

    private static native void nativeSurfaceChanged(Surface surface, int width, int height,
            float density);

    private static native void nativeSurfaceDestroyed();

    private static native void nativeInsets(int left, int top, int right, int bottom);

    private static native void nativeTouch(int device, int id, int phase, float x, float y,
            float pressure);

    private static native void nativeScroll(float x, float y);

    private static native boolean nativeKey(int code, boolean pressed, int repeat, int meta,
            int character);

    static native void nativeCompose(String text);

    static native void nativeCommit(String text, boolean composing);

    static native void nativeRecompose(int move, int delete, String text);

    static native void nativeDelete(int before, int after);

    static native void nativeMove(int by);
}
