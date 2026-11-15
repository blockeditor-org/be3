package com.be3.host;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/**
 * Takes the UnifiedPush distributor's broadcasts, with or without the activity: Android starts
 * the app's process for one when it is closed. A push is opened by be-host's library
 * (`android_push.rs`), which works from the files directory alone, on a thread of its own while
 * the broadcast is held open (`goAsync`).
 */
public final class PushReceiver extends BroadcastReceiver {
    static {
        System.loadLibrary("be_host");
    }

    static native void nativeNewEndpoint(String files, String endpoint);

    static native void nativeUnregistered(String files);

    /** The notification's title, body, organization and block, or null to drop the push. */
    static native String[] nativeMessage(String files, byte[] body);

    @Override
    public void onReceive(Context context, Intent intent) {
        String action = intent.getAction();
        if (action == null || !Push.ours(context, intent)) return;
        String files = context.getFilesDir().getAbsolutePath();
        PendingResult pending = goAsync();
        new Thread(() -> {
            try {
                switch (action) {
                    case Push.NEW_ENDPOINT: {
                        String endpoint = intent.getStringExtra("endpoint");
                        if (endpoint != null) nativeNewEndpoint(files, endpoint);
                        break;
                    }
                    case Push.MESSAGE: {
                        byte[] body = intent.getByteArrayExtra("bytesMessage");
                        if (body != null) show(context, nativeMessage(files, body));
                        Push.acknowledge(context, intent);
                        break;
                    }
                    case Push.UNREGISTERED:
                    case Push.REGISTRATION_FAILED:
                        Push.forget(context);
                        nativeUnregistered(files);
                        break;
                    default:
                        break;
                }
            } finally {
                pending.finish();
            }
        }, "be-push").start();
    }

    private static void show(Context context, String[] opened) {
        if (opened == null || opened.length != 4) return;
        Push.show(context, opened[0], opened[1], opened[2], opened[3]);
    }
}
