global N: u32 = 1000;

fn main(x: Field, inputs: pub [Field; 99]) -> pub Field {
    let mut acc = x;
    for i in 0..N {
        acc = acc * acc + inputs[i % 99];
    }
    acc
}
