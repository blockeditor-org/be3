package com.be3.beui;

import android.app.Activity;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.view.KeyEvent;
import android.view.View;
import android.view.Window;
import android.view.WindowManager;
import android.window.BackEvent;
import android.window.OnBackAnimationCallback;
import android.window.OnBackInvokedCallback;
import android.window.OnBackInvokedDispatcher;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;

public class BeuiActivity extends Activity {
    private static final String LIBRARY_NAME = "android.app.lib_name";
    static final int BACK_STARTED = 0;
    static final int BACK_PROGRESSED = 1;
    static final int BACK_CANCELLED = 2;
    static final int BACK_INVOKED = 3;
    private static final int PICK_FILE_REQUEST = 0x8E31;
    private static final int MAX_FILE_BYTES = 128 * 1024 * 1024;
    private static final int COPY_BUFFER_BYTES = 64 * 1024;

    private boolean backHandled;
    private Object backCallback;
    private long picking;

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

    void pickFile(long id, String mimeTypes) {
        if (picking != 0) BeuiView.nativeFilePicked(picking, null, null, null);
        picking = id;
        String[] types = mimeTypes.isEmpty() ? new String[0] : mimeTypes.split(",");
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType(types.length == 1 ? types[0] : "*/*");
        if (types.length > 1) intent.putExtra(Intent.EXTRA_MIME_TYPES, types);
        try {
            startActivityForResult(intent, PICK_FILE_REQUEST);
        } catch (Exception error) {
            picking = 0;
            BeuiView.nativeFilePicked(id, null, null, "No app is available to choose a file");
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        if (request != PICK_FILE_REQUEST) {
            super.onActivityResult(request, result, data);
            return;
        }
        long id = picking;
        picking = 0;
        if (id == 0) return;
        Uri uri = result == RESULT_OK && data != null ? data.getData() : null;
        if (uri == null) {
            BeuiView.nativeFilePicked(id, null, null, null);
            return;
        }
        new Thread(() -> {
            try {
                BeuiView.nativeFilePicked(id, displayName(uri), read(uri), null);
            } catch (Throwable error) {
                BeuiView.nativeFilePicked(id, null, null, "Could not read the chosen file: " + error);
            }
        }, "beui-file-picker").start();
    }

    private String displayName(Uri uri) {
        String[] columns = { OpenableColumns.DISPLAY_NAME };
        try (Cursor cursor = getContentResolver().query(uri, columns, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) {
                String name = cursor.getString(0);
                if (name != null && !name.isEmpty()) return name;
            }
        } catch (Exception ignored) {}
        String path = uri.getLastPathSegment();
        return path == null ? "" : path.substring(path.lastIndexOf('/') + 1);
    }

    private byte[] read(Uri uri) throws Exception {
        try (InputStream stream = getContentResolver().openInputStream(uri)) {
            if (stream == null) throw new IllegalStateException("it could not be opened");
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            byte[] buffer = new byte[COPY_BUFFER_BYTES];
            int read;
            while ((read = stream.read(buffer)) != -1) {
                if (bytes.size() + read > MAX_FILE_BYTES) {
                    throw new IllegalStateException("it is larger than " + (MAX_FILE_BYTES / (1024 * 1024)) + " MB");
                }
                bytes.write(buffer, 0, read);
            }
            return bytes.toByteArray();
        }
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
