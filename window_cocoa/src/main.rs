use cocoa::base::{ selector, nil, NO, id,
};
use cocoa::foundation::{
    NSAutoreleasePool, NSRect, NSPoint, NSSize, NSString,
};
use cocoa::appkit::{
    NSApp, NSWindow, NSApplication,
    NSWindowStyleMask, NSMenu, NSMenuItem,
    NSApplicationActivationPolicyRegular,
    NSBackingStoreBuffered,
};

fn main() {

    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);
        add_menu(&app);

        let window = NSWindow::alloc(nil)
            .initWithContentRect_styleMask_backing_defer_(
                NSRect::new(NSPoint::new(0., 0.), NSSize::new(500., 300.)),
                NSWindowStyleMask::NSTitledWindowMask |
                NSWindowStyleMask::NSClosableWindowMask |
                NSWindowStyleMask::NSResizableWindowMask |
                NSWindowStyleMask::NSMiniaturizableWindowMask |
                NSWindowStyleMask::NSUnifiedTitleAndToolbarWindowMask,
                NSBackingStoreBuffered,
                NO
            ).autorelease();

        window.cascadeTopLeftFromPoint_(NSPoint::new(20., 20.));
        window.center();

        let title = NSString::alloc(nil).init_str("Hello World!");
        window.setTitle_(title);
        window.makeKeyAndOrderFront_(nil);

        app.run();
    }
}

unsafe fn add_menu(app: &id) {
    // create Menu Bar
    let menubar = NSMenu::new(nil).autorelease();
    let app_menu_item = NSMenuItem::new(nil).autorelease();
    menubar.addItem_(app_menu_item);
    app.setMainMenu_(menubar);

    // create Application menu
    let app_menu = NSMenu::new(nil).autorelease();
    let quit_title = NSString::alloc(nil).init_str("Quit");
    let quit_action = selector("terminate:");
    let quit_key = NSString::alloc(nil).init_str("q");
    let quit_item = NSMenuItem::alloc(nil)
        .initWithTitle_action_keyEquivalent_(quit_title, quit_action, quit_key)
        .autorelease();
    app_menu.addItem_(quit_item);
    app_menu_item.setSubmenu_(app_menu);
}