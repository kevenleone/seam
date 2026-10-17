// Runs before the app bundle: repaints the last theme so the first frame is already right.
(function () {
    var paint;

    try {
        paint = JSON.parse(localStorage.getItem('seam.paint') || 'null');
    } catch {
        paint = null;
    }

    var root = document.documentElement;

    if (!paint || !paint.tokens) {
        root.dataset.theme = 'dark';

        return;
    }

    root.dataset.theme = paint.mode;
    root.style.colorScheme = paint.mode;
    Object.keys(paint.tokens).forEach(function (name) {
        root.style.setProperty(name, paint.tokens[name]);
    });
})();
