# Typing saved keys

The default flow is clickable: **Type** on a vault row → click the destination field → **Type into [window]** in the floating panel. The owner chose this instead of a global shortcut or long-press Ctrl+K on November 2, 2026. There is no shortcut to learn or conflict with other apps.

The auxiliary `typing` window is 380×400, always on top and non-focusable (`focusable: false`, GTK `accept_focus: false` and `focus_on_map: false`). Do not call `set_focus` on it. Clicking a button must leave the external field's input focus intact. This is the stated exception to design rule 13. The panel warns before every send because X11 cannot confirm password fields or browser origins. No site-trust preference is stored.

`src-tauri/src/typing.rs` manages one key id, a per-arm token, a destination revision, a session weak reference, a 30-second deadline and the current destination. Native polling observes window metadata only while armed; it does not record keystrokes. Every Type request compares the live focus with the destination shown in the panel. The X11 implementation in `os/linux/typing.rs` checks again for each character. The shared controller, not the backend, refuses chat and mail apps by the window class the backend reports, so a Windows backend gets the same rule. The confirmation carries the revision displayed by the panel, so a stale button cannot silently target a more recently selected window. The request cannot retarget itself or survive a lock/unlock cycle. The panel stays open with success, failure or Disarmed, until Done/Close; cancellation also interrupts an in-flight send.

`Handoff::type_key` opens a value only inside Rust. Its native callback receives bytes and a `started` callback, called immediately before the first output. Once keys are consumed and persisted there; a preflight failure keeps them. The core mutex serializes typing with replacement, forgetting and other uses. Decrypted bytes are held by the existing zeroizing `Secret` type. XTest briefly places one character in the shared keyboard mapping; the saved map is restored on errors and after each pair.

The platform interface is deliberately small: `Desktop::open`, `capture -> Option<Target>`, and `type_text(target, text, alive, started)`. `None` means one of our windows and clears the destination. Errors, including refused apps, clear it too. Only display titles and key metadata go to TypeScript. The non-Linux stub reports unavailability.

Windows follow-up for HAND-14:

- Add a Windows backend using SendInput with KEYEVENTF_UNICODE (including UTF-16 surrogate pairs); guard each pair against foreground/focused control changes and `alive()` cancellation.
- Keep the panel non-activating when clicked, including its webview. Verify this with a real app on Windows; otherwise the clickable flow cannot safely preserve the field.
- Use UI Automation IsPassword if reliable. Keep the visible-field warning for everything unknown. Do not infer a browser site from its window title.
- Refuse our windows and known chat/mail classes. Report UIPI/elevated-target failures clearly; do not claim a send succeeded merely because it was queued.
- Keep once-consumption, timeout and session checks shared. Add native tests for Unicode, focus loss, an elevated destination and the non-activating panel.

Wayland is explicitly refused, including XWayland. Future support needs the appropriate desktop portal rather than unrestricted X11 injection.
