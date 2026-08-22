# Copies the design-system CSS + fonts into ui/ds/ for the Tauri frontendDist
# (one folder must contain everything the WebView loads). design-system/ stays
# the source of truth; run this after editing it.
import os, shutil

repo = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
src = os.path.join(repo, 'design-system')
dst = os.path.join(repo, 'ui', 'ds')

items = [
    'styles.css',
    'tokens', 'foundations',
    ('components/components.css', 'components/components.css'),
    ('assets/fonts', 'assets/fonts'),
    ('assets/mark.svg', 'assets/mark.svg'),
    ('assets/mark-dark.svg', 'assets/mark-dark.svg'),
]

if os.path.isdir(dst):
    shutil.rmtree(dst)
for item in items:
    rel_src, rel_dst = item if isinstance(item, tuple) else (item, item)
    s, d = os.path.join(src, rel_src), os.path.join(dst, rel_dst)
    os.makedirs(os.path.dirname(d), exist_ok=True)
    if os.path.isdir(s):
        shutil.copytree(s, d)
    else:
        shutil.copy2(s, d)
    print('synced', rel_dst)
