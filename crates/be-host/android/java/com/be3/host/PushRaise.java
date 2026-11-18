package com.be3.host;

import android.app.Service;
import android.content.Intent;
import android.os.Binder;
import android.os.IBinder;

/**
 * The service UnifiedPush asks every app to expose (`RAISE_TO_FOREGROUND`): a distributor binds
 * it for a moment before a message, which raises this app's process enough for `PushReceiver`
 * to start. It does nothing itself.
 */
public final class PushRaise extends Service {
    private final IBinder binder = new Binder();

    @Override
    public IBinder onBind(Intent intent) {
        return binder;
    }
}
