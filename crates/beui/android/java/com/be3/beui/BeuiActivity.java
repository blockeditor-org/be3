package com.be3.beui;

import android.app.Activity;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.os.Build;
import android.os.Bundle;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;

public class BeuiActivity extends Activity {
    private static final String LIBRARY_NAME = "android.app.lib_name";

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
        setContentView(view);
        view.requestFocus();
        BeuiView.nativeCreate(this, view, getAssets(), getFilesDir().getAbsolutePath());
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
}
