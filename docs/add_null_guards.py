# -*- coding: utf-8 -*-
"""Defect #4 batch transformer v3: add `unavailable: bool` to interface structs and
insert a per-method guard `if self.unavailable { return <zero>; }` at every pub fn.

Anchor: defect-04-null-guard-deep-dive.md fix skeleton (Timeline `disabled` precedent).
v2: closed all 18 unknown zero values; added UpdateWatchHandle / NetworkingMessage /
AchievementHelper (guard via parent field); skips associated fns without self.
v3: re-emit the field separator consumed by the site regex (v2 broke literals);
Clone impls of RemotePlay/RemoteStorage covered; scripted post-fixes for
receiver-specific names, missing imports and pointer handles.
Run: python add_null_guards.py   (operates on wt-fix-null/src, prints a report)
"""
import re, io
from pathlib import Path

BASE = Path(r"C:/Users/lbx13/Documents/Qoder/2026-10-10/a96da148/wt-fix-null/src")
OUT = Path(r"C:/Users/lbx13/Documents/Qoder/2026-10-10/a96da148/null_guard_report.txt")

TARGETS = {
    'utils.rs': ['Utils'],
    'matchmaking.rs': ['Matchmaking'],
    'matchmaking_servers.rs': ['MatchmakingServers'],
    'networking.rs': ['Networking'],
    'app.rs': ['Apps'],
    'friends.rs': ['Friends', 'Friend'],
    'input.rs': ['Input'],
    'user.rs': ['User'],
    'user_stats.rs': ['UserStats'],
    'remote_play.rs': ['RemotePlay', 'RemotePlaySession'],
    'remote_storage.rs': ['RemoteStorage', 'SteamFile'],
    'screenshots.rs': ['Screenshots'],
    'ugc.rs': ['UGC', 'UpdateHandle', 'UpdateWatchHandle'],
    'networking_messages.rs': ['NetworkingMessages'],
    'networking_sockets.rs': ['NetworkingSockets', 'NetPollGroup'],
    'networking_utils.rs': ['NetworkingUtils'],
    'networking_types.rs': ['NetworkingMessage'],
}
# guard via a parent struct's field instead of a local one (no field added)
GUARD_ONLY = {
    'user_stats/stats.rs': [('AchievementHelper', 'self.parent.unavailable')],
}
# sub-handles constructed in-crate from guarded methods: propagate self.unavailable
SUBHANDLE_SITES = {
    'friends.rs': ['Friend'],
    'remote_play.rs': ['RemotePlay', 'RemotePlaySession'],
    'remote_storage.rs': ['RemoteStorage', 'SteamFile'],
    'networking_sockets.rs': ['NetPollGroup'],
    'ugc.rs': ['UpdateHandle', 'UpdateWatchHandle'],
}
# message wrappers constructed only on already-guarded receive paths: plain false
FALSE_SITES = {
    'networking_messages.rs': ['NetworkingMessage'],
    'networking_sockets.rs': ['NetworkingMessage'],
    'networking_utils.rs': ['NetworkingMessage'],
}
GETTER_FILES = ['lib.rs', 'server.rs']  # literal sites -> unavailable: <var>.is_null()

def zero_file_literal(inner):
    return ('SteamFile { unavailable: true, rs: std::ptr::null_mut(), '
            f'util: std::ptr::null_mut(), _inner: self.{inner}.clone(), name: CString::default() }}')

ZERO_SPECIAL = {
    '()': 'return;',
    'bool': 'return false;',
    'String': 'return String::new();',
    '&Self': 'return self;',
    'Self': 'return self;',
    '&[u8]': 'return &[];',
    'u32': 'return 0;', 'i32': 'return 0;', 'u64': 'return 0;', 'usize': 'return 0;',
    'i64': 'return 0;', 'f32': 'return 0.0;', 'f64': 'return 0.0;',
    'sys::InputActionSetHandle_t': 'return 0;',
    'sys::InputDigitalActionHandle_t': 'return 0;',
    'sys::InputAnalogActionHandle_t': 'return 0;',
    # repr(C) POD C data structs: all-zero is the natural "no input data" value
    'sys::InputDigitalActionData_t': 'return unsafe { std::mem::zeroed() };',
    'sys::InputAnalogActionData_t': 'return unsafe { std::mem::zeroed() };',
    'sys::InputMotionData_t': 'return unsafe { std::mem::zeroed() };',
    'SteamId': 'return SteamId::from_raw(0);',
    'AppId': 'return AppId(0);',
    'AuthTicket': 'return AuthTicket(0);',
    '(AuthTicket, Vec<u8>)': 'return (AuthTicket(0), Vec::new());',
    'UserHasLicense': 'return UserHasLicense::NoAuth;',
    'InputType': 'return InputType::Unknown;',
    'ItemState': 'return ItemState::from_bits_truncate(0);',
    'SteamResult': 'return Err(SteamError::IOFailure);',
    'NetworkingAvailabilityResult': 'return Err(NetworkingAvailabilityError::Unknown);',
    '(NetworkingConnectionState, Option<NetConnectionInfo>, Option<NetConnectionRealTimeInfo>)':
        'return (NetworkingConnectionState::None, None, None);',
    'RelayNetworkStatus':
        'return RelayNetworkStatus { availability: Err(NetworkingAvailabilityError::Unknown), '
        'is_ping_measurement_in_progress: false, network_config: Err(NetworkingAvailabilityError::Unknown), '
        'any_relay: Err(NetworkingAvailabilityError::Unknown), debugging_message: String::new() };',
    # cascade sub-handles / wrappers
    'Friend': ('return Friend { unavailable: true, id: SteamId::from_raw(0), '
               'friends: std::ptr::null_mut(), _inner: self.inner.clone() };'),
    'FriendState': 'return FriendState::Offline;',
    "stats::AchievementHelper<'_>": ('return stats::AchievementHelper { name: CString::default(), '
                                     'parent: self };'),
    'RemotePlaySession': ('return RemotePlaySession { unavailable: true, '
                          'session: RemotePlaySessionId::from_raw(0), rp: std::ptr::null_mut(), '
                          '_inner: self.inner.clone() };'),
    'SteamFile': f'return {zero_file_literal("inner")};',  # RemoteStorage receiver
    'SteamFileWriter': f'return SteamFileWriter {{ file: {zero_file_literal("_inner")}, handle: 0 }};',
    'SteamFileReader': f'return SteamFileReader {{ file: {zero_file_literal("_inner")}, offset: 0, size: 0 }};',
    'RemoteStoragePlatforms': 'return RemoteStoragePlatforms::from_bits_truncate(0);',
    'UpdateHandle': ('return UpdateHandle { unavailable: true, ugc: std::ptr::null_mut(), '
                     'inner: self.inner.clone(), handle: 0 };'),
    'UpdateWatchHandle': ('return UpdateWatchHandle { unavailable: true, ugc: std::ptr::null_mut(), '
                          '_inner: self.inner.clone(), handle: 0 };'),
    '(UpdateStatus, u64, u64)': 'return (UpdateStatus::Invalid, 0, 0);',
    'NetPollGroup': ('return NetPollGroup { unavailable: true, handle: 0, '
                     'sockets: std::ptr::null_mut(), inner: self.inner.clone(), '
                     'message_buffer: Vec::new() };'),
    'NetworkingMessage': ('return NetworkingMessage { unavailable: true, message: std::ptr::null_mut(), '
                          '_inner: self.inner.clone() };'),
    'Arc<Mutex<ServerListRequest>>':
        ('return std::sync::Arc::new(std::sync::Mutex::new(ServerListRequest { h_req: 0, '
         'released: true, mms: std::ptr::null_mut(), real: std::ptr::null_mut() }));'),
    'MessageNumber': 'return MessageNumber(0);',
    'SendFlags': 'return SendFlags::from_bits_truncate(0);',
    'NetworkingIdentity': 'return NetworkingIdentity::new();',
}
ERR_ZERO = {
    '()': 'Err(())',
    'SteamError': 'Err(SteamError::IOFailure)',
    'AuthSessionError': 'Err(AuthSessionError::InvalidTicket)',
    'CreateQueryError': 'Err(CreateQueryError)',
    'InvalidHandle': 'Err(InvalidHandle)',
    'bool': 'Err(false)',
    'NetworkingAvailabilityError': 'Err(NetworkingAvailabilityError::Unknown)',
    'ScreenshotLibraryAddError': 'Err(ScreenshotLibraryAddError::SavingFailed)',
    'MessageError': 'Err(MessageError::NullBuffer)',
}
# receiver-specific / import-specific rewrites applied after the mechanical pass
POST_FIXES = [
    ('lib.rs', 'unavailable: mms.is_null(),', 'unavailable: mm.is_null(),'),
    ('lib.rs', 'unavailable: user_stats.is_null(),', 'unavailable: us.is_null(),'),
    ('matchmaking_servers.rs', 'ServerListRequest { h_req: 0, released: true,',
     'ServerListRequest { h_req: std::ptr::null_mut(), released: true,'),
    ('networking_messages.rs', 'return Err(SteamError::IOFailure); }',
     'return Err(crate::SteamError::IOFailure); }'),
    ('networking_utils.rs', 'NetworkingAvailabilityError::Unknown',
     'crate::networking_types::NetworkingAvailabilityError::Unknown'),
]

def norm(rt):
    r = ' '.join(rt.split()).strip()
    r = re.sub(r'\s*where\s+.*$', '', r)
    r = re.sub(r",\s*\)", ')', r)          # trailing commas inside generics
    r = re.sub(r"\(\s+", '(', r)           # "( Foo," from multiline tuples
    r = re.sub(r"&'\w+\s+", '&', r)        # &'a [u8] -> &[u8]
    return r.strip()

def split_result(r):
    inner = r[len('Result<'):-1]
    depth = 0
    for i, c in enumerate(inner):
        if c in '<(': depth += 1
        elif c in '>)': depth -= 1
        elif c == ',' and depth == 0:
            return inner[:i].strip(), inner[i + 1:].strip()
    return None

def zero_for(rt):
    r = norm(rt)
    if r in ZERO_SPECIAL:
        return ZERO_SPECIAL[r]
    if r.startswith('*const'):
        return 'return std::ptr::null();'
    if r.startswith('*mut'):
        return 'return std::ptr::null_mut();'
    if r.startswith('Option<'):
        return 'return None;'
    if r.startswith('Vec<'):
        return 'return Vec::new();'
    if r.startswith('Result<'):
        p = split_result(r)
        if p:
            err = p[1].strip().rstrip(',').strip()
            if err in ERR_ZERO:
                return 'return %s;' % ERR_ZERO[err]
    return None

report = io.StringIO()
guard_count = 0
unknown = []

def is_impl_target(txt, pos):
    pre = txt[max(0, pos - 120):pos]
    return re.search(r'(?:^|[^\w:])impl(?:<[^<>]*>)?\s*(?:\w+::)*(?:for\s+)?$', pre, re.S) is not None

def find_matching(txt, i):
    depth = 0
    while i < len(txt):
        if txt[i] == '{': depth += 1
        elif txt[i] == '}':
            depth -= 1
            if depth == 0: return i
        i += 1
    return -1

def ret_type_of(txt, sig_start, body_open):
    seg = txt[sig_start:body_open]
    m = re.search(r'\)\s*(?:->\s*(.*))?$', seg, re.S)
    if m and m.group(1):
        return m.group(1)
    return '()'

def add_guards(txt, fname, n, guard_expr):
    global guard_count
    for im in re.finditer(r'impl\b(?:<[^<>]*>)?\s*', txt):
        rest = txt[im.end():]
        tm = re.match(r'(?:\w+::)*(' + re.escape(n) + r')\b', rest)
        is_for = False
        if not tm:
            tm = re.match(r'[^{\n]{0,60}?\bfor\s+(?:\w+::)*(' + re.escape(n) + r')\b', rest)
            if not tm: continue
            is_for = True
        if not is_for and not is_impl_target(txt, im.end()):
            continue
        open_brace = txt.find('{', im.end() + tm.end() - 1)
        if open_brace == -1: continue
        close = find_matching(txt, open_brace)
        if close == -1: continue
        body = txt[open_brace:close + 1]
        ins = []
        for fm in re.finditer(r'pub fn\s+(\w+)', body):
            fn_name = fm.group(1)
            sig_line_start = body.rfind('\n', 0, fm.start()) + 1
            indent = re.match(r'[ \t]*', body[sig_line_start:]).group(0)
            b_open = body.find('{', fm.end())
            if b_open == -1: continue
            sig = body[fm.start():b_open]
            if not re.search(r'\(\s*&?\s*(?:mut\s+)?self\b', sig):
                continue  # associated fn, no receiver to guard
            rt = ret_type_of(body, fm.start(), b_open)
            z = zero_for(rt)
            if z is None:
                unknown.append(f"{fname}:{n}::{fn_name} -> {norm(rt)}")
                continue
            ins.append((b_open + 1, f"\n{indent}if {guard_expr} {{ {z} }}"))
            guard_count += 1
        for off, s in sorted(ins, reverse=True):
            body = body[:off] + s + body[off:]
        txt = txt[:open_brace] + body + txt[close + 1:]
    return txt

def insert_at_literal_sites(txt, n, field_line):
    """insert field_line right after `<n> {` + newline + first field, skipping defs/impls;
    the matched field separator is re-emitted unchanged"""
    out, pos, cnt = [], 0, 0
    for m in re.finditer(r'\b' + n + r'\s*\{\n(\s*)(\w+)(\s*[:,])', txt):
        pre = txt[max(0, m.start() - 80):m.start()]
        if re.search(r'(struct\s*|impl[^{\n]*)$', pre, re.S): continue
        ins = f"{n} {{\n{m.group(1)}{field_line}\n{m.group(1)}{m.group(2)}{m.group(3)}"
        out.append(txt[pos:m.start()]); out.append(ins); pos = m.end(); cnt += 1
    out.append(txt[pos:])
    return ''.join(out), cnt

for fname, names in TARGETS.items():
    path = BASE / fname
    txt = path.read_text(encoding='utf-8')
    orig = txt
    # 1. field insertion into struct defs
    for n in names:
        pat = re.compile(r'((?:pub(?:\([^)]*\))?\s+)?struct\s+' + n + r'\b[^{]*\{)')
        m = pat.search(txt)
        if not m:
            report.write(f"!! {fname}: struct {n} NOT FOUND\n"); continue
        txt = txt[:m.end()] + '\n    pub(crate) unavailable: bool,' + txt[m.end():]
        report.write(f"++ {fname}: field added to struct {n}\n")
    # 2. guards
    for n in names:
        txt = add_guards(txt, fname, n, 'self.unavailable')
    # 3. construction sites
    for n in SUBHANDLE_SITES.get(fname, []):
        txt, cnt = insert_at_literal_sites(txt, n, 'unavailable: self.unavailable,')
        report.write(f"++ {fname}: {cnt} construction site(s) of {n} propagate self.unavailable\n")
    for n in FALSE_SITES.get(fname, []):
        txt, cnt = insert_at_literal_sites(txt, n, 'unavailable: false,')
        report.write(f"++ {fname}: {cnt} construction site(s) of {n} set unavailable: false\n")
    if txt != orig:
        path.write_text(txt, encoding='utf-8')
        report.write(f"** {fname} written\n")
    else:
        report.write(f"== {fname} unchanged\n")

for fname, items in GUARD_ONLY.items():
    path = BASE / fname
    txt = path.read_text(encoding='utf-8')
    for n, gexpr in items:
        txt = add_guards(txt, fname, n, gexpr)
        report.write(f"++ {fname}: guards on {n} via {gexpr}\n")
    path.write_text(txt, encoding='utf-8')

# 4. getter literal sites in lib.rs / server.rs
for fname in GETTER_FILES:
    path = BASE / fname
    txt = path.read_text(encoding='utf-8')
    allnames = sorted({n for ns in TARGETS.values() for n in ns} |
                      {'Friend', 'RemotePlaySession', 'SteamFile', 'NetPollGroup', 'UpdateHandle'})
    cnt_total = 0
    for n in allnames:
        out, pos, cnt = [], 0, 0
        for m in re.finditer(r'\b' + n + r'\s*\{\n(\s*)(\w+)(\s*:\s*\w+)?,', txt):
            pre = txt[max(0, m.start() - 80):m.start()]
            if re.search(r'(struct\s*|impl[^{\n]*)$', pre, re.S): continue
            var = m.group(2)
            ins = (f"{n} {{\n{m.group(1)}unavailable: {var}.is_null(),\n"
                   f"{m.group(1)}{m.group(2)}{m.group(3) or ''},")
            out.append(txt[pos:m.start()]); out.append(ins); pos = m.end(); cnt += 1
        out.append(txt[pos:])
        txt = ''.join(out)
        cnt_total += cnt
    path.write_text(txt, encoding='utf-8')
    report.write(f"** {fname}: {cnt_total} literal site(s) set unavailable: var.is_null()\n")

# 5. scripted post-fixes
for fname, old, new in POST_FIXES:
    path = BASE / fname
    t2 = path.read_text(encoding='utf-8')
    n2 = t2.count(old)
    t2 = t2.replace(old, new)
    path.write_text(t2, encoding='utf-8')
    report.write(f"** post-fix {fname}: {n2} x {old[:44]}\n")

report.write(f"\nGUARDS INSERTED: {guard_count}\n")
if unknown:
    report.write("UNKNOWN ZERO VALUES (%d):\n" % len(unknown))
    for u in unknown: report.write("  ? " + u + "\n")
res = report.getvalue()
OUT.write_text(res, encoding='utf-8')
print(res[-1600:])
print("TOTAL_CHARS", len(res))
