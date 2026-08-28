# Copies the design-system CSS + fonts into ui/ds/ for the Tauri frontendDist
# (one folder must contain everything the WebView loads). design-system/ is
# meant to be the source of truth; run this after editing it.
#
# THE CATCH, found the hard way (0.7.1): it is not the source of truth in
# practice. ui/ds/ has been edited directly and has drifted ahead — deployment
# node styles, group boundaries, and several tokens exist only there. This
# script deletes the destination before copying, so running it wholesale
# silently removed shipped styles and broke the headless renderer, which reads
# tokens out of ui/ds/.
#
# So it now REFUSES rather than clobbering: before overwriting any CSS file it
# checks that every selector and custom property already in the destination
# still exists in the source, and stops if something would be lost. Nothing is
# written when the check fails. Reconcile by hand — copy the drifted rules back
# into design-system/ — and run it again.
import os, re, shutil, sys

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

# `--foo:` declarations and selectors at the start of a line. Deliberately
# crude: this is a "did something disappear" tripwire, not a CSS parser.
DECL = re.compile(r'(--[a-z0-9-]+)\s*:', re.I)
SELECTOR = re.compile(r'^([.#][A-Za-z][\w.>\s:()\[\]="\'-]*)\s*\{', re.M)


def names(text):
    return set(DECL.findall(text)) | {s.strip() for s in SELECTOR.findall(text)}


def css_pairs():
    """(source, destination) for every .css file this would overwrite."""
    for item in items:
        rel_src, rel_dst = item if isinstance(item, tuple) else (item, item)
        s, d = os.path.join(src, rel_src), os.path.join(dst, rel_dst)
        if os.path.isdir(s):
            for root, _dirs, files in os.walk(s):
                for f in files:
                    if f.endswith('.css'):
                        full = os.path.join(root, f)
                        yield full, os.path.join(d, os.path.relpath(full, s))
        elif s.endswith('.css'):
            yield s, d


lost = []
for s, d in css_pairs():
    if not os.path.exists(d):
        continue
    with open(s, encoding='utf-8') as f:
        source = names(f.read())
    with open(d, encoding='utf-8') as f:
        dest = names(f.read())
    missing = sorted(dest - source)
    if missing:
        lost.append((os.path.relpath(d, repo), missing))

if lost:
    print('REFUSING to sync: ui/ds/ has rules design-system/ does not.\n')
    for rel, missing in lost:
        print(f'  {rel} would lose:')
        for name in missing:
            print(f'    {name}')
    print(
        '\nui/ds/ is generated, but it has been edited directly and is ahead.\n'
        'Copy those rules into design-system/ first, then run this again.'
    )
    sys.exit(1)

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
