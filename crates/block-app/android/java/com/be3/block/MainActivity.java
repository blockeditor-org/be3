package com.be3.block;

import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import com.be3.beui.BeuiActivity;
import java.io.OutputStream;

public final class MainActivity extends BeuiActivity {
    private static final int SAVE_FILE_REQUEST = 0x8E32;
    private static MainActivity current;
    private byte[] saving;

    @Override
    protected void onCreate(Bundle state) {
        current = this;
        super.onCreate(state);
    }

    @Override
    protected void onDestroy() {
        if (current == this) current = null;
        super.onDestroy();
    }

    public static boolean saveFile(String name, String mimeType, byte[] data) {
        MainActivity activity = current;
        if (activity == null) return false;
        activity.runOnUiThread(() -> activity.launchSaver(name, mimeType, data));
        return true;
    }

    private void launchSaver(String name, String mimeType, byte[] data) {
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.setType(mimeType.isEmpty() ? "application/octet-stream" : mimeType);
        intent.putExtra(Intent.EXTRA_TITLE, name);
        saving = data;
        try {
            startActivityForResult(intent, SAVE_FILE_REQUEST);
        } catch (Exception error) {
            saving = null;
            nativeFileSaved(0, "No app is available to save a file");
        }
    }

    private void finishSaving(int result, Intent data) {
        byte[] bytes = saving;
        saving = null;
        Uri uri = result == RESULT_OK && data != null ? data.getData() : null;
        if (uri == null || bytes == null) {
            nativeFileSaved(0, null);
            return;
        }
        new Thread(() -> {
            try (OutputStream stream = getContentResolver().openOutputStream(uri)) {
                if (stream == null) throw new IllegalStateException("it could not be opened");
                stream.write(bytes);
                nativeFileSaved(1, null);
            } catch (Throwable error) {
                nativeFileSaved(0, "Could not save the file: " + error);
            }
        }, "block-app-file-saver").start();
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        if (request == SAVE_FILE_REQUEST) {
            finishSaving(result, data);
            return;
        }
        super.onActivityResult(request, result, data);
    }

    private static native void nativeFileSaved(int saved, String error);
}
