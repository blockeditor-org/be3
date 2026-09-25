package com.be3.block;

import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import android.view.KeyEvent;
import com.google.androidgamesdk.GameActivity;
import com.google.androidgamesdk.gametextinput.State;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;

public final class MainActivity extends GameActivity {
    private static final int PICK_FILE_REQUEST = 0x8E31;
    private static final int MAX_FILE_BYTES = 128 * 1024 * 1024;
    private static final int COPY_BUFFER_BYTES = 64 * 1024;
    private static final int COMMAND_META =
            KeyEvent.META_CTRL_ON | KeyEvent.META_ALT_ON | KeyEvent.META_META_ON;
    private static final int MODIFIER_META = COMMAND_META | KeyEvent.META_SHIFT_ON;
    private static final int[][] MODIFIER_KEYS = {
        {KeyEvent.META_CTRL_ON, KeyEvent.KEYCODE_CTRL_LEFT},
        {KeyEvent.META_ALT_ON, KeyEvent.KEYCODE_ALT_LEFT},
        {KeyEvent.META_META_ON, KeyEvent.KEYCODE_META_LEFT},
        {KeyEvent.META_SHIFT_ON, KeyEvent.KEYCODE_SHIFT_LEFT},
    };
    private static MainActivity current;
    private int heldMeta;
    private int synthesizedMeta;

    static { System.loadLibrary("block_app_lib"); }

    @Override
    protected void onCreate(Bundle state) {
        current = this;
        super.onCreate(state);
        stateChanged(new State("", 0, 0, -1, -1), false);
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        int code = event.getKeyCode();
        int action = event.getAction();
        if (KeyEvent.isModifierKey(code)) {
            int meta = metaOf(code);
            if (action == KeyEvent.ACTION_DOWN) heldMeta |= meta;
            if (action == KeyEvent.ACTION_UP) heldMeta &= ~meta;
            return toNative(event);
        }
        if (!isCommand(event)) return super.dispatchKeyEvent(event);
        if (action == KeyEvent.ACTION_DOWN) {
            int missing = event.getMetaState() & MODIFIER_META & ~heldMeta & ~synthesizedMeta;
            synthesize(event, KeyEvent.ACTION_DOWN, missing);
            synthesizedMeta |= missing;
            return toNative(event);
        }
        boolean handled = toNative(event);
        if (action == KeyEvent.ACTION_UP) {
            synthesize(event, KeyEvent.ACTION_UP, synthesizedMeta);
            synthesizedMeta = 0;
        }
        return handled;
    }

    private static boolean isCommand(KeyEvent event) {
        if ((event.getMetaState() & COMMAND_META) != 0) return true;
        switch (event.getKeyCode()) {
            case KeyEvent.KEYCODE_TAB:
            case KeyEvent.KEYCODE_ESCAPE:
            case KeyEvent.KEYCODE_DPAD_LEFT:
            case KeyEvent.KEYCODE_DPAD_RIGHT:
            case KeyEvent.KEYCODE_DPAD_UP:
            case KeyEvent.KEYCODE_DPAD_DOWN:
            case KeyEvent.KEYCODE_MOVE_HOME:
            case KeyEvent.KEYCODE_MOVE_END:
            case KeyEvent.KEYCODE_PAGE_UP:
            case KeyEvent.KEYCODE_PAGE_DOWN:
                return true;
            default:
                return false;
        }
    }

    private static int metaOf(int code) {
        switch (code) {
            case KeyEvent.KEYCODE_CTRL_LEFT:
            case KeyEvent.KEYCODE_CTRL_RIGHT:
                return KeyEvent.META_CTRL_ON;
            case KeyEvent.KEYCODE_ALT_LEFT:
            case KeyEvent.KEYCODE_ALT_RIGHT:
                return KeyEvent.META_ALT_ON;
            case KeyEvent.KEYCODE_META_LEFT:
            case KeyEvent.KEYCODE_META_RIGHT:
                return KeyEvent.META_META_ON;
            case KeyEvent.KEYCODE_SHIFT_LEFT:
            case KeyEvent.KEYCODE_SHIFT_RIGHT:
                return KeyEvent.META_SHIFT_ON;
            default:
                return 0;
        }
    }

    private void synthesize(KeyEvent event, int action, int meta) {
        for (int[] modifier : MODIFIER_KEYS) {
            if ((meta & modifier[0]) == 0) continue;
            toNative(new KeyEvent(event.getDownTime(), event.getEventTime(), action, modifier[1], 0,
                    event.getMetaState(), event.getDeviceId(), 0, event.getFlags(),
                    event.getSource()));
        }
    }

    private boolean toNative(KeyEvent event) {
        switch (event.getAction()) {
            case KeyEvent.ACTION_DOWN:
                return onKeyDown(event.getKeyCode(), event);
            case KeyEvent.ACTION_UP:
                return onKeyUp(event.getKeyCode(), event);
            default:
                return super.dispatchKeyEvent(event);
        }
    }

    @Override
    protected void onDestroy() {
        if (current == this) current = null;
        super.onDestroy();
    }

    public static boolean pickFile(String mimeTypes) {
        MainActivity activity = current;
        if (activity == null) return false;
        activity.runOnUiThread(() -> activity.launchPicker(mimeTypes));
        return true;
    }

    private void launchPicker(String mimeTypes) {
        String[] types = mimeTypes.split(",");
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType(types.length == 1 ? types[0] : "*/*");
        if (types.length > 1) intent.putExtra(Intent.EXTRA_MIME_TYPES, types);
        try {
            startActivityForResult(intent, PICK_FILE_REQUEST);
        } catch (Exception error) {
            nativeFilePicked(null, null, "No app is available to choose a file");
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        if (request != PICK_FILE_REQUEST) {
            super.onActivityResult(request, result, data);
            return;
        }
        Uri uri = result == RESULT_OK && data != null ? data.getData() : null;
        if (uri == null) {
            nativeFilePicked(null, null, null);
            return;
        }
        new Thread(() -> {
            try {
                nativeFilePicked(displayName(uri), read(uri), null);
            } catch (Throwable error) {
                nativeFilePicked(null, null, "Could not read the chosen file: " + error);
            }
        }, "block-app-file-picker").start();
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

    private static native void nativeFilePicked(String name, byte[] data, String error);
}
