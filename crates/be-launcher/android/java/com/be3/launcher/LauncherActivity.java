package com.be3.launcher;

import android.app.PendingIntent;
import android.content.Intent;
import android.content.IntentSender;
import android.content.pm.PackageInstaller;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Bundle;
import android.view.View;
import androidx.core.graphics.Insets;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.androidgamesdk.GameActivity;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.io.OutputStream;

public final class LauncherActivity extends GameActivity {
    private static final String APP = "com.be3.block.ci";
    private static final String INSTALLED = "com.be3.launcher.INSTALLED";
    private static final int COPY_BUFFER_BYTES = 64 * 1024;
    private static LauncherActivity current;

    @Override
    protected void onCreate(Bundle state) {
        current = this;
        super.onCreate(state);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
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
            nativeInstallFinished(message == null ? "Android did not finish the change" : message);
        }
    }

    @Override
    protected void onDestroy() {
        if (current == this) current = null;
        super.onDestroy();
    }

    public static String open() {
        LauncherActivity activity = current;
        if (activity == null) return "The launcher is not open";
        Intent intent = activity.getPackageManager().getLaunchIntentForPackage(APP);
        if (intent == null) return "The app is not installed";
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        activity.runOnUiThread(() -> activity.startActivity(intent));
        return null;
    }

    public static boolean installed() {
        LauncherActivity activity = current;
        if (activity == null) return false;
        try {
            activity.getPackageManager().getPackageInfo(APP, 0);
            return true;
        } catch (PackageManager.NameNotFoundException error) {
            return false;
        }
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
            return "Could not install it: " + error;
        }
    }

    public static String uninstall() {
        LauncherActivity activity = current;
        if (activity == null) return "The launcher is not open";
        if (!installed()) {
            nativeInstallFinished(null);
            return null;
        }
        try {
            activity.getPackageManager().getPackageInstaller().uninstall(APP, activity.finished());
            return null;
        } catch (Exception error) {
            return "Could not remove the app: " + error;
        }
    }

    private void installPackage(File apk) throws Exception {
        PackageInstaller installer = getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params =
                new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        int id = installer.createSession(params);
        try (PackageInstaller.Session session = installer.openSession(id)) {
            try (InputStream in = new FileInputStream(apk);
                    OutputStream out = session.openWrite("package.apk", 0, apk.length())) {
                byte[] buffer = new byte[COPY_BUFFER_BYTES];
                int read;
                while ((read = in.read(buffer)) != -1) out.write(buffer, 0, read);
                session.fsync(out);
            }
            session.commit(finished());
        }
    }

    private IntentSender finished() {
        Intent intent = new Intent(this, LauncherActivity.class).setAction(INSTALLED);
        PendingIntent pending = PendingIntent.getActivity(this, 0, intent,
                PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_MUTABLE);
        return pending.getIntentSender();
    }

    private static native void nativeSafeAreaChanged(int left, int top, int right, int bottom);

    private static native void nativeInstallFinished(String error);
}
