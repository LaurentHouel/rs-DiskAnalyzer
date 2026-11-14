fn main() {
    // Embeds the icon shown by Explorer for the .exe (the window icon is set at runtime).
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("failed to embed the Windows icon");
    }
}
