#[macro_export]
macro_rules! hocon {
    // 顶级入口，匹配一个对象
    ({ $($content:tt)* }) => {
        $crate::value::Value::Object($crate::hocon_object!(@internal {} $($content)*))
    };
    // 顶级入口，匹配一个数组
    ([ $($content:tt)* ]) => {
        $crate::value::Value::Array($crate::hocon_array!(@internal $($content)*))
    };
    // 捕获单个表达式作为值，例如 `hocon!(10)`
    ($e:expr) => {
        $crate::value::Value::from($e)
    };
}

// 内部宏：解析HOCON对象
#[macro_export]
macro_rules! hocon_object {
    // 终止递归：当没有更多token时返回一个HashMap
    (@internal { $($k:expr => $v:expr),* } ) => {
        {
            let mut map = $ahash::ahash::HashMap::new();
            $(
                map.insert($k, $v);
            )*
            map
        }
    };

    // 规则1：匹配键值对，分隔符为 `=`
    (@internal { $($k:expr => $v:expr),* } $key:ident = $val:tt $($rest:tt)*) => {
        $crate::hocon_object!(@internal { $($k => $v,)* String::from(stringify!($key)) => $crate::hocon_value!($val) } $($rest)*)
    };

    // 规则2：匹配键值对，分隔符为 `:`
    (@internal { $($k:expr => $v:expr),* } $key:ident : $val:tt $($rest:tt)*) => {
        $crate::hocon_object!(@internal { $($k => $v,)* String::from(stringify!($key)) => $crate::hocon_value!($val) } $($rest)*)
    };

    // 规则3：匹配键值对，键带引号，分隔符为 `:`
    (@internal { $($k:expr => $v:expr),* } $key:literal : $val:tt $($rest:tt)*) => {
        $crate::hocon_object!(@internal { $($k => $v,)* $key.to_string() => $crate::hocon_value!($val) } $($rest)*)
    };
    
    // 规则4：匹配键值对，键带引号，分隔符为 `=`
    (@internal { $($k:expr => $v:expr),* } $key:literal = $val:tt $($rest:tt)*) => {
        $crate::hocon_object!(@internal { $($k => $v,)* $key.to_string() => $crate::hocon_value!($val) } $($rest)*)
    };

    // 规则5：匹配无分隔符的子对象
    (@internal { $($k:expr => $v:expr),* } $key:ident { $($sub_content:tt)* } $($rest:tt)*) => {
        $crate::hocon_object!(@internal { $($k => $v,)* String::from(stringify!($key)) => $crate::hocon!({ $($sub_content)* }) } $($rest)*)
    };

    // 规则6：处理可选逗号或换行，直接进入下一个匹配
    (@internal { $($k:expr => $v:expr),* } , $($rest:tt)*) => {
        $crate::hocon_object!(@internal { $($k => $v),* } $($rest)*)
    };
}

// 内部宏：解析HOCON数组
#[macro_export]
macro_rules! hocon_array {
    // 终止递归：返回一个Vec
    (@internal [ $($val:expr),* ] ) => {
        {
            let mut vec = $crate::HoconArray::new();
            $(
                vec.push($val);
            )*
            vec
        }
    };
    // 匹配并添加一个元素，然后继续递归
    (@internal [ $($val:expr),* ] $head:tt $($tail:tt)*) => {
        $crate::hocon_array!(@internal [ $($val,)* $crate::hocon_value!($head) ] $($tail)*)
    };
}

// 内部宏：将各种token转换为Value
#[macro_export]
macro_rules! hocon_value {
    // 处理嵌套的对象或数组
    ({ $($content:tt)* }) => {
        $crate::hocon!({ $($content)* })
    };
    ([ $($content:tt)* ]) => {
        $crate::hocon!([ $($content)* ])
    };
    // 将字面量转换为 Value 变体
    ($lit:literal) => {
        $crate::value::Value::from($lit)
    };
    // 将布尔值转换为 Value 变体
    ($bool:ident) => {
        $crate::value::Value::from($bool)
    };
    // 捕获表达式，将其转换为 Value
    ($e:expr) => {
        $crate::value::Value::from($e)
    };
    // 处理 HOCON null
    (null) => {
        $crate::value::Value::Null
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_hocon_macro() {}
}