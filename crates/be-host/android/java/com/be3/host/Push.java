package com.be3.host;

import android.app.BroadcastOptions;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.content.pm.ResolveInfo;
import android.os.Build;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * Push notifications through a UnifiedPush distributor
 * (decisions/android_push_via_unifiedpush.md): finding one, registering and unregistering with
 * it by plain intents, and showing what `PushReceiver` opened. The distributor and the token it
 * knows this app by are kept in this app's preferences; the endpoint and the keys pushes are
 * encrypted to are kept by the Rust side, in the host state.
 */
final class Push {
    static final String REGISTER = "org.unifiedpush.android.distributor.REGISTER";
    static final String UNREGISTER = "org.unifiedpush.android.distributor.UNREGISTER";
    static final String MESSAGE_ACK = "org.unifiedpush.android.distributor.MESSAGE_ACK";
    static final String NEW_ENDPOINT = "org.unifiedpush.android.connector.NEW_ENDPOINT";
    static final String MESSAGE = "org.unifiedpush.android.connector.MESSAGE";
    static final String UNREGISTERED = "org.unifiedpush.android.connector.UNREGISTERED";
    static final String REGISTRATION_FAILED = "org.unifiedpush.android.connector.REGISTRATION_FAILED";

    static final String EXTRA_ORGANIZATION = "com.be3.host.organization";
    static final String EXTRA_BLOCK = "com.be3.host.block";
    /**
     * The activity-alias a pressed notification starts `MainActivity` through. It is not
     * exported, so only this app can start it: `MainActivity` honours a block to open only from
     * an intent naming it, never from one another app sends to the exported activity.
     */
    static final String OPEN_ALIAS = "com.be3.host.OpenNotification";

    /** What `pushState` answers, as `be_host_base::push_device::PushDevice` reads it. */
    static final int UNSUPPORTED = 0;
    static final int OFF = 1;
    static final int ON = 2;
    static final int DENIED = 3;

    private static final String PREFERENCES = "be3-push";
    private static final String TOKEN = "token";
    private static final String DISTRIBUTOR = "distributor";
    private static final String ASKED = "asked";
    private static final String CHANNEL = "messages";

    private Push() {}

    private static SharedPreferences preferences(Context context) {
        return context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE);
    }

    /** The installed distributors' packages, by the receivers that take REGISTER. */
    static List<String> distributors(Context context) {
        List<String> names = new ArrayList<>();
        for (ResolveInfo info : context.getPackageManager().queryBroadcastReceivers(new Intent(REGISTER), 0)) {
            String name = info.activityInfo.packageName;
            if (!name.equals(context.getPackageName()) && !names.contains(name)) names.add(name);
        }
        return names;
    }

    /**
     * The distributor to use without asking: the one chosen before while it is installed, or
     * the only one. Null when there is none, or several and none chosen yet.
     */
    static String chosenDistributor(Context context) {
        List<String> installed = distributors(context);
        String kept = preferences(context).getString(DISTRIBUTOR, null);
        if (kept != null && installed.contains(kept)) return kept;
        return installed.size() == 1 ? installed.get(0) : null;
    }

    /** A distributor's name as the launcher shows it, or its package. */
    static String label(Context context, String distributor) {
        PackageManager packages = context.getPackageManager();
        try {
            return packages.getApplicationLabel(packages.getApplicationInfo(distributor, 0)).toString();
        } catch (PackageManager.NameNotFoundException error) {
            return distributor;
        }
    }

    static boolean mayNotify(Context context) {
        return Build.VERSION.SDK_INT < 33
                || context.checkSelfPermission("android.permission.POST_NOTIFICATIONS")
                        == PackageManager.PERMISSION_GRANTED;
    }

    static int state(Context context) {
        SharedPreferences kept = preferences(context);
        if (!mayNotify(context) && kept.getBoolean(ASKED, false)) return DENIED;
        if (distributors(context).isEmpty()) return UNSUPPORTED;
        return kept.getString(TOKEN, null) != null && mayNotify(context) ? ON : OFF;
    }

    static void markAsked(Context context) {
        preferences(context).edit().putBoolean(ASKED, true).apply();
    }

    /**
     * Asks `distributor` for an endpoint, with the server's VAPID key when known, and keeps it as
     * the chosen one. It answers NEW_ENDPOINT or REGISTRATION_FAILED to `PushReceiver`.
     */
    static void register(Context context, String distributor, String vapid) {
        SharedPreferences kept = preferences(context);
        String token = kept.getString(TOKEN, null);
        if (token == null || !distributor.equals(kept.getString(DISTRIBUTOR, null))) {
            token = UUID.randomUUID().toString();
        }
        kept.edit().putString(TOKEN, token).putString(DISTRIBUTOR, distributor).apply();
        Intent intent = new Intent(REGISTER).setPackage(distributor).putExtra("token", token);
        intent.putExtra("message", "Notifications of new messages");
        if (vapid != null) intent.putExtra("vapid", vapid);
        send(context, intent);
    }

    /** Tells the distributor to drop this app's endpoint, and forgets the token. */
    static void unregister(Context context) {
        SharedPreferences kept = preferences(context);
        String token = kept.getString(TOKEN, null);
        String distributor = kept.getString(DISTRIBUTOR, null);
        kept.edit().remove(TOKEN).apply();
        if (token == null || distributor == null) return;
        send(context, new Intent(UNREGISTER).setPackage(distributor).putExtra("token", token));
    }

    /** Forgets the token after the distributor dropped it. */
    static void forget(Context context) {
        preferences(context).edit().remove(TOKEN).apply();
    }

    /** Whether a broadcast is from the distributor about this app's registration. */
    static boolean ours(Context context, Intent intent) {
        String token = preferences(context).getString(TOKEN, null);
        return token != null && token.equals(intent.getStringExtra("token"));
    }

    static void acknowledge(Context context, Intent message) {
        String id = message.getStringExtra("id");
        String distributor = preferences(context).getString(DISTRIBUTOR, null);
        if (id == null || distributor == null) return;
        send(context, new Intent(MESSAGE_ACK).setPackage(distributor)
                .putExtra("token", message.getStringExtra("token"))
                .putExtra("id", id));
    }

    /**
     * Sends to the distributor, naming this app: the distributor learns who asks from the
     * broadcast's identity from Android 14, and from a PendingIntent before.
     */
    private static void send(Context context, Intent intent) {
        if (Build.VERSION.SDK_INT >= 34) {
            BroadcastOptions options = BroadcastOptions.makeBasic();
            options.setShareIdentityEnabled(true);
            context.sendBroadcast(intent, null, options.toBundle());
            return;
        }
        Intent dummy = new Intent("org.unifiedpush.dummy_app").setPackage("org.unifiedpush.dummy_app");
        intent.putExtra("pi", PendingIntent.getBroadcast(context, 0, dummy, PendingIntent.FLAG_IMMUTABLE));
        context.sendBroadcast(intent);
    }

    /**
     * Shows a notification. `organization` and `block` are empty for one naming nothing; a
     * later one about the same block replaces the earlier, and pressing one opens its block.
     */
    static void show(Context context, String title, String body, String organization, String block) {
        NotificationManager manager = context.getSystemService(NotificationManager.class);
        if (manager == null || !mayNotify(context)) return;
        Notification.Builder builder;
        if (Build.VERSION.SDK_INT >= 26) {
            manager.createNotificationChannel(new NotificationChannel(
                    CHANNEL, "Messages", NotificationManager.IMPORTANCE_HIGH));
            builder = new Notification.Builder(context, CHANNEL);
        } else {
            builder = new Notification.Builder(context).setPriority(Notification.PRIORITY_HIGH);
        }
        Intent open = new Intent()
                .setComponent(new ComponentName(context, OPEN_ALIAS))
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP
                        | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        if (!block.isEmpty()) {
            open.putExtra(EXTRA_ORGANIZATION, organization).putExtra(EXTRA_BLOCK, block);
        }
        String tag = block.isEmpty() ? "new-message" : block;
        PendingIntent pressed = PendingIntent.getActivity(context, tag.hashCode(), open,
                PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        builder.setSmallIcon(android.R.drawable.stat_notify_chat)
                .setContentTitle(title)
                .setContentText(body)
                .setStyle(new Notification.BigTextStyle().bigText(body))
                .setAutoCancel(true)
                .setContentIntent(pressed);
        manager.notify(tag, 0, builder.build());
    }
}
