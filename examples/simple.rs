use futures::StreamExt;
use select_loop::select_loop;

#[tokio::main]
async fn main() {
    let string_counter = futures::stream::iter(100..).map(|x| x.to_string());
    let num_counter = futures::stream::iter(0..);
    let ready = futures::future::ready(42);
    let never = futures::future::pending::<u32>();

    // statically known that all branches set a value
    let mut latest;

    select_loop! {
        S string_counter => |number| latest = number,
        S num_counter => |number| {
            latest = number.to_string();

            if number == 20 {
                break;
            }
        },
        F ready => |number| latest = number.to_string(),
        F never => |number| latest = number.to_string(),

        @after => println!("{latest}"),
    };
}
