use select_loop::select_loop;

#[tokio::main]
async fn main() {
    let num_counter = futures::stream::iter(0..100);

    let out = select_loop! {

        // When all streams are exhausted the loop breaks with the following
        // value. If no `@exhausted` branch exists then the loop breaks with `()`.
        // If there are multiple `@exhausted` branches then only the last value
        // will be returned.
        @exhausted => 5,
        @exhausted => 10,
        S num_counter => |number| {
            if number == 120 {
                // It is possible to break with value as long as the value has
                // the same type as what is returned by the last `@exhausted`
                // branch. No value can be breaked explicitly if there is no
                // `@exhausted` branch.
                break 5;
            }
        },
    };

    assert_eq!(out, 10);
}
