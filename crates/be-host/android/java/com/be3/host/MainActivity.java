package com.be3.host;

import android.app.AlertDialog;
import android.content.ComponentName;
import android.content.Intent;
import android.os.Build;
import android.os.Bundle;
import com.be3.beui.BeuiActivity;
import java.util.List;

public final class MainActivity extends BeuiActivity {
    private static final int NOTIFY_REQUEST = 0x8E33;

    private String pendingVapid;
    private boolean registerWhenAnswered;

    static native void nativeOpen(String organization, String block);

    static native void nativePushChanged();

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        openPressed(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        openPressed(intent);
    }

    /**
     * A pressed notification names its block: the host opens it (`pressed_notifications`). Only
     * an intent through the alias, which no other app may start, is a pressed notification.
     */
    private void openPressed(Intent intent) {
        if (intent == null) return;
        ComponentName component = intent.getComponent();
        if (component == null || !Push.OPEN_ALIAS.equals(component.getClassName())) return;
        String organization = intent.getStringExtra(Push.EXTRA_ORGANIZATION);
        String block = intent.getStringExtra(Push.EXTRA_BLOCK);
        if (organization != null && block != null) nativeOpen(organization, block);
    }

    /** Whether pushes show here, as `Push.state` answers; called by be-host on its thread. */
    int pushState() {
        return Push.state(this);
    }

    /**
     * Turns pushes on: asks for the permission to notify where Android needs it, then registers
     * with the distributor, asking which when several are installed and none was chosen.
     * Called by be-host on its own thread.
     */
    void pushEnable(String vapid) {
        runOnUiThread(() -> {
            if (Build.VERSION.SDK_INT >= 33 && !Push.mayNotify(this)) {
                pendingVapid = vapid;
                registerWhenAnswered = true;
                Push.markAsked(this);
                requestPermissions(new String[] { "android.permission.POST_NOTIFICATIONS" }, NOTIFY_REQUEST);
                return;
            }
            register(vapid);
        });
    }

    /** Registers with the chosen distributor, or asks which one with a plain chooser. */
    private void register(String vapid) {
        String chosen = Push.chosenDistributor(this);
        if (chosen != null) {
            Push.register(this, chosen, vapid);
            nativePushChanged();
            return;
        }
        List<String> installed = Push.distributors(this);
        if (installed.isEmpty()) {
            nativePushChanged();
            return;
        }
        String[] labels = new String[installed.size()];
        for (int at = 0; at < labels.length; at++) labels[at] = Push.label(this, installed.get(at));
        new AlertDialog.Builder(this)
                .setTitle("Receive notifications through")
                .setItems(labels, (dialog, which) -> {
                    Push.register(this, installed.get(which), vapid);
                    nativePushChanged();
                })
                .setOnCancelListener(dialog -> nativePushChanged())
                .show();
    }

    void pushDisable() {
        Push.unregister(this);
        nativePushChanged();
    }

    @Override
    public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request != NOTIFY_REQUEST) return;
        boolean register = registerWhenAnswered && Push.mayNotify(this);
        String vapid = pendingVapid;
        registerWhenAnswered = false;
        pendingVapid = null;
        if (register) {
            register(vapid);
        } else {
            nativePushChanged();
        }
    }
}
