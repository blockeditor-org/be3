package com.be3.host;

import android.content.Intent;
import android.os.Build;
import android.os.Bundle;
import com.be3.beui.BeuiActivity;

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

    /** A pressed notification names its block: the host opens it (`pressed_notifications`). */
    private void openPressed(Intent intent) {
        if (intent == null) return;
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
     * with the distributor. Called by be-host on its own thread.
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
            Push.register(this, vapid);
            nativePushChanged();
        });
    }

    void pushDisable() {
        Push.unregister(this);
        nativePushChanged();
    }

    @Override
    public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request != NOTIFY_REQUEST) return;
        if (registerWhenAnswered && Push.mayNotify(this)) Push.register(this, pendingVapid);
        registerWhenAnswered = false;
        pendingVapid = null;
        nativePushChanged();
    }
}
