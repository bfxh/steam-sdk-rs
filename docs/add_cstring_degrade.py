# -*- coding: utf-8 -*-
"""Defect #5 transformer: eliminate CString::new(...).unwrap()/expect() panics in
public APIs without changing any signature (roadmap batch judgment: #5 可立即做,
zero-break like #2E/#3/#4; the UGC builder chain `-> Self` stays with defect #6).

Strategy: replace `let NAME = CString::new(EXPR).unwrap();` statements with
`let Some(NAME) = cstring(EXPR) else { return ZERO; };` where ZERO comes from a
per-site map keyed by (file, line). Helper `crate::cstring()` returns None on
interior NUL. Manual sites (closures, Option maps, struct literals, constants)
are reported for hand-fixing.

Run from a96da148: python add_cstring_degrade.py  (operates on wt-fix-cstring/src)
"""
import re
from pathlib import Path

BASE = Path(r"C:/Users/lbx13/Documents/Qoder/2026-10-10/a96da148/wt-fix-cstring/src")
OUT = Path(r"C:/Users/lbx13/Documents/Qoder/2026-10-10/a96da148/cstring_fix_report.txt")

# --- per-site zero values (file, line) -> C return expr ----------------------
# lines from the master-base census (cstring_census.txt, verified 77 panic sites)
Z = {}
def z(f, line, ret):
    Z.setdefault(f, {})[line] = ret

# app.rs
z('app.rs', 167, 'return String::new();')
# friends.rs
for ln in (124, 133, 166, 189, 473): z('friends.rs', ln, 'return;')
z('friends.rs', 206, 'return false;')
z('friends.rs', 207, 'return false;')
z('friends.rs', 492, 'return None;')
# input.rs
z('input.rs', 71, 'return false;')
z('input.rs', 79, 'return 0;')
z('input.rs', 155, 'return 0;')
z('input.rs', 161, 'return 0;')
# matchmaking.rs
z('matchmaking.rs', 156, 'return None;')
z('matchmaking.rs', 195, 'return false;')
z('matchmaking.rs', 196, 'return false;')
z('matchmaking.rs', 209, 'return false;')
z('matchmaking.rs', 217, 'return;')
z('matchmaking.rs', 218, 'return;')
z('matchmaking.rs', 239, 'return None;')
z('matchmaking.rs', 407, 'return self;')
z('matchmaking.rs', 408, 'return self;')
z('matchmaking.rs', 433, 'return self;')
z('matchmaking.rs', 458, 'return self;')
# networking_sockets.rs (762 close + 1076 drop-constant are manual)
z('networking_sockets.rs', 816, 'return;')
# networking_types.rs (NetworkingConfigEntry::new_string)
z('networking_types.rs', 1641, 'return Self::new_uninitialized_config_value();')
# remote_storage.rs file() -> manual
# screenshots.rs
z('screenshots.rs', 142, 'return Err(ScreenshotLibraryAddError::SavingFailed);')
# server.rs (init + 10 unit setters)
z('server.rs', None, None)  # placeholder: init line resolved at run time below
SERVER_INIT_ZERO = ('return Err(SteamAPIInitError::Generic('
                    'String::from("version string contains a NUL byte")));')
# unit setters: lines resolved dynamically (see UNIT_FNS)
# timeline.rs
z('timeline.rs', 81, 'return;')
z('timeline.rs', 120, 'return;')
z('timeline.rs', 121, 'return;')
z('timeline.rs', 122, 'return;')
# user.rs
z('user.rs', None, None)  # auth ticket line resolved dynamically
USER_AUTH_ZERO = 'return AuthTicket(0);'
# user_stats.rs: callback APIs feed Err to the callback instead of dropping it
z('user_stats.rs', 21, 'cb(Err(SteamError::IOFailure)); return;')
z('user_stats.rs', 52, 'cb(Err(SteamError::IOFailure)); return;')
z('user_stats.rs', 550, 'return Err(());')
z('user_stats.rs', 573, 'return Err(());')
z('user_stats.rs', 592, 'return Err(());')
z('user_stats.rs', 615, 'return Err(());')
# user_stats.rs 635 (achievement ctor) -> manual
# user_stats/stats.rs
z('user_stats/stats.rs', None, None)  # resolved dynamically
STATS_ATTR_ZERO = 'return Err(());'
# utils.rs (253 show_gamepad_text_input description; 254 Option map + 320 closure manual)
z('utils.rs', 253, 'return false;')

UNIT_FNS = {  # file -> fn names whose CString sites degrade with `return;`
    'server.rs': ['set_product', 'set_game_description', 'set_game_data', 'log_on',
                  'set_mod_dir', 'set_map_name', 'set_server_name', 'set_game_tags',
                  'set_key_value'],
}

# files whose enclosing-fn lookup needs a dynamic (file, fn) -> zero table
DYN_ZERO = {}  # (file, fn) -> zero

report = []
manual = []
converted = 0

def chain_span(t, start):
    """end offset of the CString::new(...)...() method chain starting at start"""
    i = t.find('(', start); d = 1; i += 1
    while i < len(t) and d > 0:
        if t[i] == '(': d += 1
        elif t[i] == ')': d -= 1
        i += 1
    end = i
    while True:
        mm = re.match(r'\s*\.\s*(\w+)\s*\(', t[end:])
        if not mm: break
        j = end + mm.end(); d2 = 1
        while j < len(t) and d2 > 0:
            if t[j] == '(': d2 += 1
            elif t[j] == ')': d2 -= 1
            j += 1
        end = j
    return end

def arg_text(t, start):
    """raw source text of the first argument of CString::new( ... )"""
    i = t.find('(', start) + 1
    d = 1; j = i
    while j < len(t):
        if t[j] == '(': d += 1
        elif t[j] == ')':
            d -= 1
            if d == 0: break
        j += 1
    return t[i:j]

for f in sorted({p for p in BASE.rglob('*.rs')}):
    rel = f.relative_to(BASE).as_posix()
    txt = f.read_text(encoding='utf-8')
    # dynamic zero tables
    dyn = {}
    m = re.search(r'let version = CString::new\(version\)\.unwrap\(\);', txt)
    if rel == 'server.rs' and m:
        dyn[m.start()] = SERVER_INIT_ZERO
    m = re.search(r'let c_str = CString::new\(identity\)\.unwrap\(\);', txt)
    if rel == 'user.rs' and m:
        dyn[m.start()] = USER_AUTH_ZERO
    m = re.search(r'let c_str = CString::new\(name\)\.unwrap\(\);', txt)
    if rel == 'user_stats/stats.rs' and m:
        dyn[m.start()] = STATS_ATTR_ZERO
    if rel in UNIT_FNS:
        for fn in UNIT_FNS[rel]:
            for fm in re.finditer(r'fn ' + fn + r'\(', txt):
                # find fn body span
                b = txt.find('{', fm.end())
                d = 1; j = b + 1
                while j < len(txt):
                    if txt[j] == '{': d += 1
                    elif txt[j] == '}':
                        d -= 1
                        if d == 0: break
                    j += 1
                body = txt[b:j]
                for cm in re.finditer(r'CString::new\(', body):
                    dyn[b + cm.start()] = 'return;'
    sites = []
    for m in re.finditer(r'CString::new\(', txt):
        end = chain_span(txt, m.start())
        seg = txt[m.start():end]
        if '.unwrap()' not in seg and '.expect(' not in seg:
            continue
        sites.append((m.start(), end, m.start()))
    # process from end to keep offsets stable
    for start, end, _ in sorted(sites, reverse=True):
        line = txt[:start].count('\n') + 1
        if rel == 'ugc.rs':
            # builder chain (`-> Self`) belongs to defect #6; init_for_game_server
            # (bool) is in the site map below
            back = txt.rfind('\n    pub fn ', 0, start)
            sig = txt[back:back + 400] if back >= 0 else ''
            if re.search(r'\)\s*->\s*Self\s*\{', sig):
                report.append(f"SKIP #6 builder  {rel}:{line}")
                continue
        zero = None
        zmap = Z.get(rel, {})
        if line in zmap:
            zero = zmap[line]
        elif rel == 'ugc.rs':
            zero = 'return false;'  # init_for_game_server
        else:
            # dynamic matches recorded by search position
            zero = dyn.get(start)
        if zero is None:
            manual.append(f"{rel}:{line}: {(' '.join(txt[start:end].split()))[:100]}")
            continue
        # pattern A: `let NAME = <chain>;` with only whitespace around
        pre = txt[:start]
        lm = re.search(r'let\s+(mut\s+)?(\w+)\s*=\s*$', pre)
        if not lm:
            manual.append(f"{rel}:{line} (no let-pattern): {(' '.join(txt[start:end].split()))[:90]}")
            continue
        post = txt[end:]
        sm = re.match(r'\s*;', post)
        if not sm:
            manual.append(f"{rel}:{line} (no stmt-end): {(' '.join(txt[start:end].split()))[:90]}")
            continue
        name = lm.group(2)
        arg = arg_text(txt, start).strip()
        new_stmt = (f"let Some({name}) = cstring({arg}) "
                    f"else {{ {zero} }};")
        txt = txt[:lm.start()] + new_stmt + txt[end + sm.end():]
        converted += 1
        report.append(f"FIX  {rel}:{line} -> {zero[:48]}")
    f.write_text(txt, encoding='utf-8')

# helper into lib.rs (once)
lib = BASE / 'lib.rs'
t = lib.read_text(encoding='utf-8')
if 'pub(crate) fn cstring(' not in t:
    anchor = '// A note about thread-safety:'
    helper = '''/// Converts a caller-provided string into a `CString`, returning `None` when it
/// contains an interior NUL byte.
///
/// Public APIs use this to stay panic-free on invalid input: instead of
/// panicking on `CString::new(...).unwrap()`, callers degrade (skip the call,
/// return `false`/`None`, or report an error) the same way the Steam SDK
/// treats invalid input.
pub(crate) fn cstring(s: impl AsRef<str>) -> Option<CString> {
    CString::new(s.as_ref()).ok()
}

'''
    assert t.count(anchor) == 1
    t = t.replace(anchor, helper + anchor)
    lib.write_text(t, encoding='utf-8')
    report.append("HELPER cstring() added to lib.rs")

res = "CONVERTED: %d\n\n%s\n\nMANUAL (%d):\n%s\n" % (
    converted, '\n'.join(report), len(manual), '\n'.join(manual) or '  none')
OUT.write_text(res, encoding='utf-8')
print(res[-2600:])
