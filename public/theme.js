// Runs before the app bundle so the first paint already uses the saved theme.
(function () {
    var theme;

    try {
        theme = JSON.parse(localStorage.getItem('seam.settings') || '{}').theme;
    } catch {
        theme = undefined;
    }

    var dark =
        theme !== 'light' &&
        (theme !== 'auto' || matchMedia('(prefers-color-scheme: dark)').matches);

    document.documentElement.dataset.theme = dark ? 'dark' : 'light';
})();
