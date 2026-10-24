//! `runtime/collation` и `runtime/locale` — перенос `CollationNatives` и
//! `LocaleNatives` JVM. Ключи сортировки даёт закреплённая ICU 76.1
//! (Unicode 16, CLDR 46) из каталога `OKKAMA_ICU_LIBDIR`; языки платформы —
//! `CFLocaleCopyPreferredLanguages` на macOS, пусто на прочих POSIX.
#![allow(unsafe_code)]

use super::support::{byte_array, failure, int, string, string_array};
use super::Error;
use crate::{Value, abi::Native};
use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::OnceLock;

const COLLATION: &str = "runtime/collation@1::collation_key/7";

fn collation_failure(native: Native, code: &str, message: &str) -> Error {
    failure("core.collation", code, native, message)
}

/// Библиотека, открытая на всё время процесса (как `Arena.global()`).
struct Library(*mut c_void);

// SAFETY: дескриптор dlopen не привязан к потоку.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

impl Library {
    fn open(path: &std::path::Path) -> Option<Self> {
        let path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        // SAFETY: путь с завершающим нулём.
        let handle = unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        (!handle.is_null()).then_some(Self(handle))
    }

    fn symbol(&self, name: &str) -> Option<*mut c_void> {
        let name = CString::new(name).ok()?;
        // SAFETY: действительный дескриптор и имя.
        let address = unsafe { libc::dlsym(self.0, name.as_ptr()) };
        (!address.is_null()).then_some(address)
    }
}

type Status = i32;

#[allow(clippy::type_complexity)]
struct Icu {
    get_version: unsafe extern "C" fn(*mut u8),
    unicode_version: unsafe extern "C" fn(*mut u8),
    cldr_version: unsafe extern "C" fn(*mut u8, *mut Status),
    open: unsafe extern "C" fn(*const c_char, *mut Status) -> *mut c_void,
    close: unsafe extern "C" fn(*mut c_void),
    set_attribute: unsafe extern "C" fn(*mut c_void, i32, i32, *mut Status),
    sort_key: unsafe extern "C" fn(*mut c_void, *const u16, i32, *mut u8, i32) -> i32,
    uca_version: unsafe extern "C" fn(*mut c_void, *mut u8),
    legacy_type: unsafe extern "C" fn(*const c_char, *const c_char) -> *const c_char,
    keyword_values: unsafe extern "C" fn(*const c_char, *const c_char, i8, *mut Status) -> *mut c_void,
    next: unsafe extern "C" fn(*mut c_void, *mut i32, *mut Status) -> *const c_char,
    enum_close: unsafe extern "C" fn(*mut c_void),
}

/// Загрузка ICU: успех или код и текст отказа (запоминается, как у JVM).
fn load() -> Result<Icu, (&'static str, &'static str)> {
    let unavailable = (
        "unsupported",
        "Pinned ICU 76.1 is unavailable; run tools/build-icu-collation.sh or set OKKAMA_ICU_LIBDIR",
    );
    let directory = crate::host::environment().get("OKKAMA_ICU_LIBDIR").cloned().ok_or(unavailable)?;
    let suffix = if cfg!(target_os = "macos") { "dylib" } else if cfg!(unix) { "so" } else { "dll" };
    let directory = std::path::Path::new(&directory);
    let uc = directory.join(format!("libicuuc.{suffix}"));
    let i18n = directory.join(format!("libicui18n.{suffix}"));
    if !uc.is_file() || !i18n.is_file() {
        return Err(unavailable);
    }
    let missing = ("unsupported", "ICU does not export the pinned 76.1 API");
    let common = Library::open(&uc).ok_or(missing)?;
    let library = Library::open(&i18n).ok_or(missing)?;
    let find = |name: &str| -> Result<*mut c_void, (&'static str, &'static str)> {
        let name = format!("{name}_76");
        library.symbol(&name).or_else(|| common.symbol(&name)).ok_or(missing)
    };
    // SAFETY: прототипы — из заголовков ICU 76.
    let icu = unsafe {
        Icu {
            get_version: std::mem::transmute::<*mut c_void, _>(find("u_getVersion")?),
            unicode_version: std::mem::transmute::<*mut c_void, _>(find("u_getUnicodeVersion")?),
            cldr_version: std::mem::transmute::<*mut c_void, _>(find("ulocdata_getCLDRVersion")?),
            open: std::mem::transmute::<*mut c_void, _>(find("ucol_open")?),
            close: std::mem::transmute::<*mut c_void, _>(find("ucol_close")?),
            set_attribute: std::mem::transmute::<*mut c_void, _>(find("ucol_setAttribute")?),
            sort_key: std::mem::transmute::<*mut c_void, _>(find("ucol_getSortKey")?),
            uca_version: std::mem::transmute::<*mut c_void, _>(find("ucol_getUCAVersion")?),
            legacy_type: std::mem::transmute::<*mut c_void, _>(find("uloc_toLegacyType")?),
            keyword_values: std::mem::transmute::<*mut c_void, _>(find("ucol_getKeywordValuesForLocale")?),
            next: std::mem::transmute::<*mut c_void, _>(find("uenum_next")?),
            enum_close: std::mem::transmute::<*mut c_void, _>(find("uenum_close")?),
        }
    };
    std::mem::forget(common);
    std::mem::forget(library);
    let version = |info: [u8; 4]| format!("{}.{}.{}.{}", info[0], info[1], info[2], info[3]);
    let mut info = [0u8; 4];
    // SAFETY: буфер UVersionInfo из четырёх байтов.
    unsafe { (icu.get_version)(info.as_mut_ptr()) };
    let library_version = version(info);
    unsafe { (icu.unicode_version)(info.as_mut_ptr()) };
    let unicode = version(info);
    let mut status = 0;
    unsafe { (icu.cldr_version)(info.as_mut_ptr(), &mut status) };
    if status > 0 {
        return Err(("unsupported", "Cannot read pinned ICU version"));
    }
    if library_version != "76.1.0.0" || unicode != "16.0.0.0" || version(info) != "46.0.0.0" {
        return Err(("unsupported", "ICU/Unicode/CLDR data version mismatch"));
    }
    Ok(icu)
}

fn icu(native: Native) -> Result<&'static Icu, Error> {
    static ICU: OnceLock<Result<Icu, (&'static str, &'static str)>> = OnceLock::new();
    match ICU.get_or_init(load) {
        Ok(icu) => Ok(icu),
        Err((code, message)) => Err(collation_failure(native, code, message)),
    }
}

fn c_text(pointer: *const c_char) -> Option<String> {
    // SAFETY: ICU возвращает строку с завершающим нулём или NULL.
    (!pointer.is_null()).then(|| unsafe { CStr::from_ptr(pointer) }.to_string_lossy().into_owned())
}

/// Java `\p{Nd}{255}`: 255 десятичных цифр подряд.
fn long_digit_run(text: &str) -> bool {
    let mut run = 0;
    for character in text.chars() {
        if crate::data::category(u32::from(character)) == "Nd" {
            run += 1;
            if run >= 255 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

fn small(value: &crate::IntValue) -> i32 {
    value.to_i64().filter(|v| i32::try_from(*v).is_ok()).map_or(-99, |v| v as i32)
}

fn collation_key(native: Native, args: &[Value]) -> Result<Value, Error> {
    let (text, locale, collation) = (string(&args[0])?, string(&args[1])?, string(&args[2])?);
    let (strength, case_first) = (small(int(&args[3])?), small(int(&args[4])?));
    let (numeric, normalization) = (args[5].as_bool()?, args[6].as_bool()?);
    let locale_valid = (1..=255).contains(&locale.len()) && locale.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let collation_valid = collation.is_empty()
        || ((3..=64).contains(&collation.len())
            && collation.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'));
    if !locale_valid || !collation_valid || !matches!(strength, 0 | 1 | 2 | 3 | 15) {
        return Err(collation_failure(native, "invalid_data", "Invalid collation coordinates"));
    }
    if !matches!(case_first, -1 | 24 | 25) {
        return Err(collation_failure(native, "invalid_data", "Invalid collation policy"));
    }
    if text.chars().count() > 8192 {
        return Err(collation_failure(native, "resource_exhausted", "Collation text exceeds 8192 scalars"));
    }
    if numeric && long_digit_run(text) {
        return Err(collation_failure(native, "invalid_data", "Numeric runs exceed ICU limit of 254 digits"));
    }
    let api = icu(native)?;
    let mut id = if locale == "und" { "root".to_string() } else { locale.replace('-', "_") };
    let keyword = CString::new("collation").expect("no NUL");
    let mut status: Status = 0;
    if !collation.is_empty() {
        let value = CString::new(collation).expect("checked");
        // SAFETY: обе строки с завершающим нулём.
        let Some(kind) = c_text(unsafe { (api.legacy_type)(keyword.as_ptr(), value.as_ptr()) }) else {
            return Err(collation_failure(native, "invalid_data", "Unknown Unicode collation type"));
        };
        let locale_id = CString::new(id.as_str()).expect("checked");
        // SAFETY: строки с нулём, status — действительный указатель.
        let values = unsafe { (api.keyword_values)(keyword.as_ptr(), locale_id.as_ptr(), 0, &mut status) };
        if status > 0 || values.is_null() {
            return Err(collation_failure(native, "invalid_data", "Cannot enumerate locale collations"));
        }
        let mut found = false;
        let mut length = 0;
        loop {
            // SAFETY: перечисление ICU открыто.
            let Some(value) = c_text(unsafe { (api.next)(values, &mut length, &mut status) }) else {
                break;
            };
            found |= value == kind;
        }
        unsafe { (api.enum_close)(values) };
        if status > 0 || !found {
            return Err(collation_failure(native, "invalid_data", "Unsupported collation for locale"));
        }
        id = format!("{id}@collation={kind}");
    }
    status = 0;
    let locale_id = CString::new(id).expect("checked");
    // SAFETY: строка с нулём, status — действительный указатель.
    let collator = unsafe { (api.open)(locale_id.as_ptr(), &mut status) };
    if status > 0 || collator.is_null() {
        if !collator.is_null() {
            unsafe { (api.close)(collator) };
        }
        return Err(collation_failure(native, "invalid_data", "Cannot open requested collation"));
    }
    struct Close(&'static Icu, *mut c_void);
    impl Drop for Close {
        fn drop(&mut self) {
            // SAFETY: коллатор открыт и закрывается один раз.
            unsafe { (self.0.close)(self.1) };
        }
    }
    let _guard = Close(api, collator);
    let mut version = [0u8; 4];
    unsafe { (api.uca_version)(collator, version.as_mut_ptr()) };
    if version != [16, 0, 0, 0] {
        return Err(collation_failure(native, "unsupported", "UCA data version mismatch"));
    }
    let settings = [
        (5, strength),
        (2, case_first),
        (7, if numeric { 17 } else { 16 }),
        (4, if normalization { 17 } else { 16 }),
    ];
    for (attribute, value) in settings {
        status = 0;
        unsafe { (api.set_attribute)(collator, attribute, value, &mut status) };
        if status > 0 {
            return Err(collation_failure(native, "invalid_data", "Invalid collation attribute"));
        }
    }
    let input: Vec<u16> = text.encode_utf16().collect();
    let units = input.len() as i32;
    let pointer = if input.is_empty() { [0u16].as_ptr() } else { input.as_ptr() };
    // SAFETY: длина совпадает с буфером UTF-16.
    let size = unsafe { (api.sort_key)(collator, pointer, units, std::ptr::null_mut(), 0) };
    if size <= 0 || size > 262_144 {
        return Err(collation_failure(native, "resource_exhausted", "Collation key exceeds 262144 bytes"));
    }
    let mut key = vec![0u8; size as usize];
    let actual = unsafe { (api.sort_key)(collator, pointer, units, key.as_mut_ptr(), size) };
    if actual != size || key[key.len() - 1] != 0 {
        return Err(collation_failure(native, "invalid_data", "Inconsistent ICU sort key"));
    }
    Ok(byte_array(key))
}

#[cfg(target_os = "macos")]
fn preferred_language_tags(native: Native) -> Result<Value, Error> {
    let locale_failure = |code: &str, message: &str| failure("core.locale", code, native, message);
    let path = std::path::Path::new("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation");
    let unexpected = || locale_failure("unexpected", "Cannot capture platform language preferences");
    let library = Library::open(path).ok_or_else(unexpected)?;
    let find = |name: &str| library.symbol(name).ok_or_else(unexpected);
    type Copy = unsafe extern "C" fn() -> *const c_void;
    type Count = unsafe extern "C" fn(*const c_void) -> isize;
    type At = unsafe extern "C" fn(*const c_void, isize) -> *const c_void;
    type Length = unsafe extern "C" fn(*const c_void) -> isize;
    type CString_ = unsafe extern "C" fn(*const c_void, *mut c_char, isize, u32) -> u8;
    type Release = unsafe extern "C" fn(*const c_void);
    // SAFETY: прототипы CoreFoundation.
    let (copy, count, at, length, cstring, release) = unsafe {
        (
            std::mem::transmute::<*mut c_void, Copy>(find("CFLocaleCopyPreferredLanguages")?),
            std::mem::transmute::<*mut c_void, Count>(find("CFArrayGetCount")?),
            std::mem::transmute::<*mut c_void, At>(find("CFArrayGetValueAtIndex")?),
            std::mem::transmute::<*mut c_void, Length>(find("CFStringGetLength")?),
            std::mem::transmute::<*mut c_void, CString_>(find("CFStringGetCString")?),
            std::mem::transmute::<*mut c_void, Release>(find("CFRelease")?),
        )
    };
    std::mem::forget(library);
    let array = unsafe { copy() };
    if array.is_null() {
        return Err(locale_failure("unexpected", "No Apple language snapshot"));
    }
    struct Guard(*const c_void, Release);
    impl Drop for Guard {
        fn drop(&mut self) {
            // SAFETY: массив получен Copy-функцией и освобождается один раз.
            unsafe { (self.1)(self.0) };
        }
    }
    let _guard = Guard(array, release);
    let size = unsafe { count(array) };
    if !(0..=64).contains(&size) {
        return Err(locale_failure("resource_exhausted", "Too many platform languages"));
    }
    let mut out = Vec::new();
    for index in 0..size {
        let string = unsafe { at(array, index) };
        let units = unsafe { length(string) };
        if !(0..=255).contains(&units) {
            return Err(locale_failure("invalid_data", "Platform language length"));
        }
        let mut buffer = vec![0 as c_char; units as usize * 4 + 1];
        // kCFStringEncodingUTF8 = 0x08000100.
        let ok = unsafe { cstring(string, buffer.as_mut_ptr(), buffer.len() as isize, 0x0800_0100) };
        if ok == 0 {
            return Err(locale_failure("invalid_data", "Cannot convert platform language to UTF8"));
        }
        let value = c_text(buffer.as_ptr()).unwrap_or_default();
        if value.encode_utf16().count() > 255 {
            return Err(locale_failure("invalid_data", "Invalid preferred language scalar"));
        }
        out.push(value);
    }
    Ok(string_array(out))
}

#[cfg(not(target_os = "macos"))]
fn preferred_language_tags(_native: Native) -> Result<Value, Error> {
    Ok(string_array(Vec::<String>::new()))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    match native.symbol() {
        COLLATION => collation_key(native, args),
        "runtime/locale@1::preferred_language_tags/0" => preferred_language_tags(native),
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
