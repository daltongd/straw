/*
NOTE:
this macro is meant to simplify definitions of functions that are later bound to a
lua module.
most importantly they make it easier to change lua arguments for those functions
`(name, id): (String, u8)` -> `(name, id, path): (String, u8, String)`
id more cumbersome to do than:
`name: String, id: u8` -> `name: String, id: u8, path: String`
as well as it's easier to read it this way

NOTE:
usage example:
```rust
function! {
    exists(path: String) -> bool {
        let path = crate::fs::expand_path(&path)?;
        Ok(PathBuf::from(path).exists())
    }
}
```

*/

macro_rules! function {
    ($name:ident() $b:block) => {
        #[allow(unused_parens, unused_variables)]
        pub fn $name(lua: &mlua::Lua) -> crate::result::LuaResult<()> {
            $b
            Ok(())
        }
    };
    ($name:ident() -> $ret:ty $b:block) => {
        #[allow(unused_parens, unused_variables)]
        pub fn $name(lua: &mlua::Lua) -> crate::result::LuaResult<$ret> { $b }
    };

    ($name:ident($($i:ident: $t:ty), *) $b:block) => {
        #[allow(unused_parens, unused_variables)]
        pub fn $name(lua: &mlua::Lua, ($($i), *): ($($t), *)) -> crate::result::LuaResult<()> {
            $b
            Ok(())
        }
    };
    ($name:ident($($i:ident: $t:ty), *) -> $ret:ty $b:block) => {
        #[allow(unused_parens, unused_variables)]
        pub fn $name(lua: &mlua::Lua, ($($i), *): ($($t), *)) -> crate::result::LuaResult<$ret> { $b }
    };
}
pub(crate) use function;

// expand with: `cargo watch -q -c -x 'expand --lib lua::macros::expand'`
#[allow(unreachable_code)]
mod expand {
    function!(_with_args(_test: String, _x: i32) {
        todo!();
    });

    function!(_no_args() {
        todo!();
    });
}
