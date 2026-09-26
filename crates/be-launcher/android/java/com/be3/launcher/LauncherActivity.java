package com.be3.launcher;

import android.app.ActivityManager;
import android.app.PendingIntent;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.net.Uri;
import android.os.Bundle;
import android.os.Process;
import android.view.KeyEvent;
import android.view.View;
import androidx.activity.BackEventCompat;
import androidx.activity.OnBackPressedCallback;
import androidx.core.graphics.Insets;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.be3.block.MainActivity;
import com.google.androidgamesdk.GameActivity;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.List;

public final class LauncherActivity extends GameActivity {
    private static final String BUILD_PROCESS = ":build";
    private static final String INSTALLED = "com.be3.launcher.INSTALLED";
    private static final int COPY_BUFFER_BYTES = 64 * 1024;
    private static final int BACK_STARTED = 0;
    private static final int BACK_PROGRESSED = 1;
    private static final int BACK_CANCELLED = 2;
    private static final int BACK_INVOKED = 3;
    private static LauncherActivity current;
    private final OnBackPressedCallback back = new OnBackPressedCallback(false) {
        @Override
        public void handleOnBackStarted(BackEventCompat event) {
            nativeBack(BACK_STARTED, event.getProgress(), event.getSwipeEdge());
        }

        @Override
        public void handleOnBackProgressed(BackEventCompat event) {
            nativeBack(BACK_PROGRESSED, event.getProgress(), event.getSwipeEdge());
        }

        @Override
        public void handleOnBackCancelled() {
            nativeBack(BACK_CANCELLED, 0f, 0);
        }

        @Override
        public void handleOnBackPressed() {
            nativeBack(BACK_INVOKED, 1f, 0);
        }
    };

    @Override
    protected void onCreate(Bundle state) {
        current = this;
        super.onCreate(state);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        getOnBackPressedDispatcher().addCallback(this, back);
    }

    public void setBackHandled(boolean handled) {
        runOnUiThread(() -> back.setEnabled(handled));
    }

    @Override
    public boolean dispatchKeyEvent(KeyEvent event) {
        if (event.getKeyCode() == KeyEvent.KEYCODE_BACK) {
            if (event.getAction() == KeyEvent.ACTION_UP && !event.isCanceled()) {
                getOnBackPressedDispatcher().onBackPressed();
            }
            return true;
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    public WindowInsetsCompat onApplyWindowInsets(View view, WindowInsetsCompat insets) {
        WindowInsetsCompat applied = super.onApplyWindowInsets(view, insets);
        Insets safe = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() | WindowInsetsCompat.Type.displayCutout());
        nativeSafeAreaChanged(safe.left, safe.top, safe.right, safe.bottom);
        return applied;
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        if (!INSTALLED.equals(intent.getAction())) return;
        int status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            Intent confirm = intent.getParcelableExtra(Intent.EXTRA_INTENT);
            if (confirm != null) {
                startActivity(confirm);
                return;
            }
        }
        if (status == PackageInstaller.STATUS_SUCCESS) {
            nativeInstallFinished(null);
        } else {
            String message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
            nativeInstallFinished(message == null ? "The launcher was not installed" : message);
        }
    }

    @Override
    protected void onDestroy() {
        if (current == this) current = null;
        super.onDestroy();
    }

    public static boolean launch(String build, String data) {
        LauncherActivity activity = current;
        if (activity == null) return false;
        activity.runOnUiThread(() -> {
            activity.stopBuild();
            Intent intent = new Intent(activity, MainActivity.class)
                    .putExtra(MainActivity.EXTRA_BUILD, build)
                    .putExtra(MainActivity.EXTRA_DATA, data)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
            activity.startActivity(intent);
        });
        return true;
    }

    public static boolean stop() {
        LauncherActivity activity = current;
        if (activity == null) return false;
        activity.stopBuild();
        return true;
    }

    public static boolean openUrl(String url) {
        LauncherActivity activity = current;
        if (activity == null) return false;
        activity.runOnUiThread(() -> {
            try {
                activity.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
            } catch (Exception ignored) {}
        });
        return true;
    }

    public static String install(String apk) {
        LauncherActivity activity = current;
        if (activity == null) return "The launcher is not open";
        try {
            activity.installPackage(new File(apk));
            return null;
        } catch (Exception error) {
            return "Could not install the launcher: " + error;
        }
    }

    private void stopBuild() {
        ActivityManager manager = (ActivityManager) getSystemService(ACTIVITY_SERVICE);
        List<ActivityManager.RunningAppProcessInfo> processes = manager.getRunningAppProcesses();
        if (processes == null) return;
        for (ActivityManager.RunningAppProcessInfo process : processes) {
            if (process.processName.endsWith(BUILD_PROCESS)) Process.killProcess(process.pid);
        }
    }

    private void installPackage(File apk) throws Exception {
        PackageInstaller installer = getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params =
                new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        int id = installer.createSession(params);
        try (PackageInstaller.Session session = installer.openSession(id)) {
            try (InputStream in = new FileInputStream(apk);
                    OutputStream out = session.openWrite("launcher.apk", 0, apk.length())) {
                byte[] buffer = new byte[COPY_BUFFER_BYTES];
                int read;
                while ((read = in.read(buffer)) != -1) out.write(buffer, 0, read);
                session.fsync(out);
            }
            Intent intent = new Intent(this, LauncherActivity.class).setAction(INSTALLED);
            PendingIntent pending = PendingIntent.getActivity(this, 0, intent,
                    PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_MUTABLE);
            session.commit(pending.getIntentSender());
        }
    }

    private static native void nativeBack(int phase, float progress, int edge);

    private static native void nativeSafeAreaChanged(int left, int top, int right, int bottom);

    private static native void nativeInstallFinished(String error);
}
