use futures::StreamExt;
use select_loop::select_loop;

#[tokio::main]
async fn main() {
    let counter = futures::stream::iter(100..).map(|x| x.to_string());
    let counter2 = futures::stream::iter(0..);
    let mut storage = vec![];

    select_loop! {
        counter => |number| storage.push(number),
        counter2 => |number| {
            storage.push(number.to_string());

            if number == 20 {
                break;
            }
        },
    };

    println!("{storage:#?}");
}
