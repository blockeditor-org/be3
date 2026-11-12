package com.be3.beui;

import android.app.Activity;
import android.app.KeyguardManager;
import android.content.Context;
import android.content.res.AssetManager;
import android.content.res.Configuration;
import android.graphics.Insets;
import android.os.Build;
import android.os.PowerManager;
import android.provider.Settings;
import android.view.DisplayCutout;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsAnimation;
import android.view.WindowInsetsController;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityManager;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.FrameLayout;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

public final class BeuiView extends SurfaceView implements SurfaceHolder.Callback2 {
    private static final long REDRAW_WAIT_SECONDS = 1;
    private static final int TOUCH_START = 0;
    private static final int TOUCH_MOVE = 1;
    private static final int TOUCH_END = 2;
    private static final int TOUCH_CANCEL = 3;
    private static final int PEN_MOVE = 0;
    private static final int PEN_DOWN = 1;
    private static final int PEN_UP = 2;
    private static final int PEN_GONE = 3;
    private static final int PEN_TIP = 0;
    private static final int PEN_BARREL = 1;
    private static int penButton = PEN_TIP;

    private final InputMethodManager input;
    private boolean keyboard;
    // Whether the soft keyboard is on screen, and whether it was when the window
    // last lost focus, as it does when the phone locks.
    private boolean keyboardVisible;
    private boolean keyboardWhenUnfocused;
    // While the keyboard slides, the insets follow the animation's frames rather
    // than jumping to where it ends (ImeAnimation).
    private boolean keyboardSliding;
    private WindowInsets latestInsets;
    private BeuiInputConnection connection;
    private ImeText ime = new ImeText(0, "", 0, 0, -1, -1, 0);
    private long imeSerial;

    BeuiView(Context context) {
        super(context);
        input = (InputMethodManager) context.getSystemService(Context.INPUT_METHOD_SERVICE);
        getHolder().addCallback(this);
        setFocusable(true);
        setFocusableInTouchMode(true);
        if (Build.VERSION.SDK_INT >= 30) setWindowInsetsAnimationCallback(new ImeAnimation(this));
    }

    @Override
    public void surfaceCreated(SurfaceHolder holder) {}

    @Override
    public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        reportPreferences(getResources().getConfiguration());
        nativeSurfaceChanged(holder.getSurface(), width, height,
                getResources().getDisplayMetrics().density);
    }

    // Android shows the window once drawingFinished runs, which beui does after it
    // presents a frame to the surface, so unlocking never shows a blank or stale one.
    @Override
    public void surfaceRedrawNeededAsync(SurfaceHolder holder, Runnable drawingFinished) {
        if (!nativeRedrawNeeded(drawingFinished)) drawingFinished.run();
    }

    // Before API 26 Android asks only this, on the UI thread, and shows the window
    // when it returns. It waits a bounded time: a stale frame beats an ANR.
    @Override
    public void surfaceRedrawNeeded(SurfaceHolder holder) {
        if (Build.VERSION.SDK_INT >= 26) return;
        CountDownLatch drawn = new CountDownLatch(1);
        if (!nativeRedrawNeeded(drawn::countDown)) return;
        try {
            drawn.await(REDRAW_WAIT_SECONDS, TimeUnit.SECONDS);
        } catch (InterruptedException interrupted) {
            Thread.currentThread().interrupt();
        }
    }

    @Override
    protected void onConfigurationChanged(Configuration configuration) {
        super.onConfigurationChanged(configuration);
        reportPreferences(configuration);
    }

    @Override
    public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (!focused) {
            keyboardWhenUnfocused = keyboard && keyboardUpWhenLocking();
            return;
        }
        reportPreferences(getResources().getConfiguration());
        // Locking the phone hides the keyboard, and nothing else asks for it again.
        if (keyboard && keyboardWhenUnfocused) post(this::showKeyboard);
        keyboardWhenUnfocused = false;
    }

    // Whether the keyboard was up as the window lost focus to the lock screen. From
    // API 30 the insets say whether it was up, so one the person put away stays
    // away; before, nothing says so, so it comes back only when the screen locked
    // or went off, not when the notification shade took focus.
    private boolean keyboardUpWhenLocking() {
        if (Build.VERSION.SDK_INT >= 30) return keyboardVisible;
        KeyguardManager keyguard =
                (KeyguardManager) getContext().getSystemService(Context.KEYGUARD_SERVICE);
        PowerManager power = (PowerManager) getContext().getSystemService(Context.POWER_SERVICE);
        return (keyguard != null && keyguard.isKeyguardLocked())
                || (power != null && !power.isInteractive());
    }

    private void showKeyboard() {
        if (!keyboard) return;
        requestFocus();
        if (Build.VERSION.SDK_INT >= 30) {
            WindowInsetsController controller = getWindowInsetsController();
            if (controller != null) {
                controller.show(WindowInsets.Type.ime());
                return;
            }
        }
        input.showSoftInput(this, 0);
    }

    private void reportPreferences(Configuration configuration) {
        int night = configuration.uiMode & Configuration.UI_MODE_NIGHT_MASK;
        float animators = Settings.Global.getFloat(getContext().getContentResolver(),
                Settings.Global.ANIMATOR_DURATION_SCALE, 1f);
        nativePreferences(night == Configuration.UI_MODE_NIGHT_YES,
                night == Configuration.UI_MODE_NIGHT_NO, animators == 0f);
    }

    @Override
    public void surfaceDestroyed(SurfaceHolder holder) {
        nativeSurfaceDestroyed();
    }

    @Override
    public WindowInsets onApplyWindowInsets(WindowInsets insets) {
        if (Build.VERSION.SDK_INT >= 30) {
            latestInsets = insets;
            keyboardVisible = insets.isVisible(WindowInsets.Type.ime());
            // These are where the keyboard's slide ends; ImeAnimation reports the way there.
            if (!keyboardSliding) reportInsets(insets);
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

    private static void reportInsets(WindowInsets insets) {
        Insets safe = insets.getInsets(
                WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
        Insets ime = insets.getInsets(WindowInsets.Type.ime());
        nativeInsets(safe.left, safe.top, safe.right, Math.max(safe.bottom, ime.bottom));
    }

    // Reports the insets of each frame of the keyboard's slide, so the layout moves
    // with the keyboard instead of jumping before or after it.
    private static final class ImeAnimation extends WindowInsetsAnimation.Callback {
        private final BeuiView view;

        ImeAnimation(BeuiView view) {
            super(DISPATCH_MODE_STOP);
            this.view = view;
        }

        private static boolean slidesKeyboard(WindowInsetsAnimation animation) {
            return (animation.getTypeMask() & WindowInsets.Type.ime()) != 0;
        }

        @Override
        public void onPrepare(WindowInsetsAnimation animation) {
            if (slidesKeyboard(animation)) view.keyboardSliding = true;
        }

        @Override
        public WindowInsets onProgress(WindowInsets insets,
                List<WindowInsetsAnimation> running) {
            if (view.keyboardSliding) reportInsets(insets);
            return insets;
        }

        @Override
        public void onEnd(WindowInsetsAnimation animation) {
            if (!slidesKeyboard(animation)) return;
            view.keyboardSliding = false;
            if (view.latestInsets != null) reportInsets(view.latestInsets);
        }
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
            case MotionEvent.ACTION_POINTER_DOWN:
                touch(event, event.getActionIndex(), TOUCH_START);
                return true;
            case MotionEvent.ACTION_MOVE:
                for (int sample = 0; sample <= event.getHistorySize(); sample++) {
                    for (int index = 0; index < event.getPointerCount(); index++) {
                        touch(event, index, TOUCH_MOVE, sample);
                    }
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
        touch(event, index, phase, event.getHistorySize());
    }

    private static void touch(MotionEvent event, int index, int phase, int sample) {
        if (isPen(event, index)) {
            pen(event, index, PEN_PHASES[phase], sample);
            return;
        }
        nativeTouch(event.getDeviceId(), event.getPointerId(index), phase,
                axis(event, MotionEvent.AXIS_X, index, sample),
                axis(event, MotionEvent.AXIS_Y, index, sample),
                axis(event, MotionEvent.AXIS_PRESSURE, index, sample), ago(event, sample));
    }

    private static float axis(MotionEvent event, int axis, int index, int sample) {
        if (sample < event.getHistorySize()) {
            return event.getHistoricalAxisValue(axis, index, sample);
        }
        return event.getAxisValue(axis, index);
    }

    private static long ago(MotionEvent event, int sample) {
        boolean precise = Build.VERSION.SDK_INT >= 34;
        long at;
        if (sample < event.getHistorySize()) {
            at = precise ? event.getHistoricalEventTimeNanos(sample)
                    : event.getHistoricalEventTime(sample) * 1_000_000L;
        } else {
            at = precise ? event.getEventTimeNanos() : event.getEventTime() * 1_000_000L;
        }
        return Math.max(0L, System.nanoTime() - at);
    }

    private static final int[] PEN_PHASES = {PEN_DOWN, PEN_MOVE, PEN_UP, PEN_GONE};

    private static boolean isPen(MotionEvent event, int index) {
        int tool = event.getToolType(index);
        return tool == MotionEvent.TOOL_TYPE_STYLUS || tool == MotionEvent.TOOL_TYPE_ERASER;
    }

    private static void pen(MotionEvent event, int index, int phase) {
        pen(event, index, phase, event.getHistorySize());
    }

    private static void pen(MotionEvent event, int index, int phase, int sample) {
        double tilt = axis(event, MotionEvent.AXIS_TILT, index, sample);
        double orientation = axis(event, MotionEvent.AXIS_ORIENTATION, index, sample);
        float tiltX = (float) Math.toDegrees(Math.atan(Math.sin(-orientation) * Math.tan(tilt)));
        float tiltY = (float) Math.toDegrees(Math.atan(Math.cos(-orientation) * Math.tan(tilt)));
        if (phase == PEN_DOWN) {
            boolean barrel = (event.getButtonState() & MotionEvent.BUTTON_STYLUS_PRIMARY) != 0;
            penButton = barrel ? PEN_BARREL : PEN_TIP;
        }
        nativePen(phase, penButton, axis(event, MotionEvent.AXIS_X, index, sample),
                axis(event, MotionEvent.AXIS_Y, index, sample),
                axis(event, MotionEvent.AXIS_PRESSURE, index, sample), tiltX, tiltY,
                event.getToolType(index) == MotionEvent.TOOL_TYPE_ERASER, ago(event, sample));
    }

    @Override
    public boolean onGenericMotionEvent(MotionEvent event) {
        if (isPen(event, 0)) {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_HOVER_ENTER:
                case MotionEvent.ACTION_HOVER_MOVE:
                    for (int sample = 0; sample <= event.getHistorySize(); sample++) {
                        pen(event, 0, PEN_MOVE, sample);
                    }
                    return true;
                case MotionEvent.ACTION_HOVER_EXIT:
                    pen(event, 0, PEN_GONE);
                    return true;
                default:
                    break;
            }
        }
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
        return nativeKey(event.getKeyCode(), event.getScanCode(), pressed,
                event.getRepeatCount(), meta, event.getUnicodeChar(meta), event.getUnicodeChar(0));
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
        info.initialSelStart = ime.index(ime.selectionStart);
        info.initialSelEnd = ime.index(ime.selectionEnd);
        connection = new BeuiInputConnection(this, ime);
        return connection;
    }

    long nextImeSerial() {
        return ++imeSerial;
    }

    void syncIme() {
        if (connection != null && ime.serial >= imeSerial) connection.sync(ime);
    }

    public void setImeText(long start, String text, long selectionStart, long selectionEnd,
            long composingStart, long composingEnd, long serial) {
        ImeText state = new ImeText(start, text, selectionStart, selectionEnd, composingStart,
                composingEnd, serial);
        post(() -> {
            ime = state;
            syncIme();
        });
    }

    static final class ImeText {
        final long start;
        final String text;
        final long selectionStart;
        final long selectionEnd;
        final long composingStart;
        final long composingEnd;
        final long serial;

        ImeText(long start, String text, long selectionStart, long selectionEnd,
                long composingStart, long composingEnd, long serial) {
            this.start = start;
            this.text = text;
            this.selectionStart = selectionStart;
            this.selectionEnd = selectionEnd;
            this.composingStart = composingStart;
            this.composingEnd = composingEnd;
            this.serial = serial;
        }

        int index(long offset) {
            long bytes = offset - start;
            int index = 0;
            while (index < text.length() && bytes > 0) {
                int next = index + Character.charCount(text.codePointAt(index));
                bytes -= BeuiInputConnection.utf8(text, index, next);
                index = next;
            }
            return index;
        }
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

    public void setBackHandled(boolean handled) {
        post(() -> ((BeuiActivity) getContext()).setBackHandled(handled));
    }

    public void setLightBars(boolean light) {
        post(() -> ((BeuiActivity) getContext()).setLightBars(light));
    }

    public void finishActivity() {
        post(() -> ((Activity) getContext()).finish());
    }

    public void pickFile(long id, String mimeTypes) {
        post(() -> ((BeuiActivity) getContext()).pickFile(id, mimeTypes));
    }

    public void saveFile(long id, String name, String mimeType, byte[] data) {
        post(() -> ((BeuiActivity) getContext()).saveFile(id, name, mimeType, data));
    }

    static final class Host extends FrameLayout {
        private final AccessibilityManager accessibility;

        Host(Context context, BeuiView view) {
            super(context);
            accessibility = (AccessibilityManager) context
                    .getSystemService(Context.ACCESSIBILITY_SERVICE);
            addView(view);
        }

        @Override
        public boolean requestSendAccessibilityEvent(View child, AccessibilityEvent event) {
            return accessibility.isEnabled() && super.requestSendAccessibilityEvent(child, event);
        }
    }

    static native void nativeCreate(Activity activity, BeuiView view, AssetManager assets,
            String files);

    static native void nativeDestroy(boolean finishing);

    static native void nativeFocus(boolean focused);

    static native void nativeLifecycle(int change);

    static native void nativeBack(int phase, float progress, int edge);

    private static native void nativeSurfaceChanged(Surface surface, int width, int height,
            float density);

    private static native void nativeSurfaceDestroyed();

    private static native boolean nativeRedrawNeeded(Runnable drawingFinished);

    private static native void nativeInsets(int left, int top, int right, int bottom);

    private static native void nativePreferences(boolean dark, boolean light, boolean reduceMotion);

    private static native void nativeTouch(int device, int id, int phase, float x, float y,
            float pressure, long agoNanos);

    private static native void nativePen(int phase, int button, float x, float y, float pressure,
            float tiltX, float tiltY, boolean eraser, long agoNanos);

    private static native void nativeScroll(float x, float y);

    private static native boolean nativeKey(int code, int scanCode, boolean pressed, int repeat,
            int meta, int character, int layoutCharacter);

    static native void nativeIme(int kind, long first, long second, String text, long serial);

    static native void nativeFilePicked(long id, String name, byte[] data, String error);

    static native void nativeFileSaved(long id, boolean saved, String error);
}
