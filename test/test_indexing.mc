fn print_int(i: u32) {}

fn main() {
    let arr: u32[4];
    let v: *u32 = (&arr[0]) as *u32;

    arr[0] = 10;
    arr[1] = 11;
    arr[2] = 12;
    arr[3] = 13;

    print_int(arr[2]);
    print_int(v[3]);
}