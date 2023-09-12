pub fn override_if_value<T: PartialEq>(value: T) -> impl Fn(&mut T, T) {
    move |orig, new| {
        if orig == &value {
            *orig = new;
        }
    }
}

pub fn override_default<T: Default + PartialEq>() -> impl Fn(&mut T, T) {
    override_if_value(T::default())
}
