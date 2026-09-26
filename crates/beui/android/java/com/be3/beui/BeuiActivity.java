package com.be3.beui;

import android.app.Activity;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;
import android.view.KeyEvent;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;
import android.window.BackEvent;
import android.window.OnBackAnimationCallback;
import android.window.OnBackInvokedCallback;
import android.window.OnBackInvokedDispatcher;

public class BeuiActivity extends Activity {
    private static final String LIBRARY_NAME = "android.app.lib_name";
    static final int BACK_STARTED = 0;
    static final int BACK_PROGRESSED = 1;
    static final int BACK_CANCELLED = 2;
    static final int BACK_INVOKED = 3;

    private boolean backHandled;
    private Object backCallback;

    protected void loadNativeLibrary() {
        try {
            ActivityInfo info = getPackageManager()
                    .getActivityInfo(getComponentName(), PackageManager.GET_META_DATA);
            System.loadLibrary(info.metaData.getString(LIBRARY_NAME));
        } catch (PackageManager.NameNotFoundException error) {
            throw new IllegalStateException(error);
        }
    }

    @Override
    protected void onCreate(Bundle state) {
        loadNativeLibrary();
        super.onCreate(state);
        Window window = getWindow();
        window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
                | WindowManager.LayoutParams.SOFT_INPUT_STATE_HIDDEN);
        if (Build.VERSION.SDK_INT >= 30) {
            window.setDecorFitsSystemWindows(false);
        } else {
            window.getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                    | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                    | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
        }
        BeuiView view = new BeuiView(this);
        setContentView(new BeuiView.Host(this, view));
        view.requestFocus();
        BeuiView.nativeCreate(this, view, getAssets(), getFilesDir().getAbsolutePath());
    }

    void setBackHandled(boolean handled) {
        if (handled == backHandled) return;
        backHandled = handled;
        if (Build.VERSION.SDK_INT < 33) return;
        if (handled) {
            backCallback = Back.register(this);
        } else {
            Back.unregister(this, backCallback);
            backCallback = null;
        }
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        if (event.getKeyCode() == KeyEvent.KEYCODE_BACK && backHandled) {
            if (event.getAction() == KeyEvent.ACTION_UP && !event.isCanceled()) {
                BeuiView.nativeBack(BACK_INVOKED, 1f, 0);
            }
            return true;
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        BeuiView.nativeFocus(focused);
    }

    @Override
    protected void onDestroy() {
        BeuiView.nativeDestroy(isFinishing());
        super.onDestroy();
    }

    private static final class Back {
        static Object register(Activity activity) {
            OnBackInvokedCallback callback = Build.VERSION.SDK_INT >= 34
                    ? new Animated()
                    : () -> BeuiView.nativeBack(BACK_INVOKED, 1f, 0);
            activity.getOnBackInvokedDispatcher().registerOnBackInvokedCallback(
                    OnBackInvokedDispatcher.PRIORITY_DEFAULT, callback);
            return callback;
        }

        static void unregister(Activity activity, Object callback) {
            if (callback == null) return;
            activity.getOnBackInvokedDispatcher()
                    .unregisterOnBackInvokedCallback((OnBackInvokedCallback) callback);
        }
    }

    private static final class Animated implements OnBackAnimationCallback {
        @Override
        public void onBackStarted(BackEvent event) {
            BeuiView.nativeBack(BACK_STARTED, event.getProgress(), event.getSwipeEdge());
        }

        @Override
        public void onBackProgressed(BackEvent event) {
            BeuiView.nativeBack(BACK_PROGRESSED, event.getProgress(), event.getSwipeEdge());
        }

        @Override
        public void onBackCancelled() {
            BeuiView.nativeBack(BACK_CANCELLED, 0f, 0);
        }

        @Override
        public void onBackInvoked() {
            BeuiView.nativeBack(BACK_INVOKED, 1f, 0);
        }
    }
}
