import { println } from "std/io";
import {
    abs,
    fabs,
    floor,
    ceil,
    sqrt,
    min,
    max,
    rand_int,
    clamp,
    gcd,
    lcm,
    round,
    trunc,
    sign,
    sin,
    cos,
    tan,
    asin,
    acos,
    atan,
    atan2,
    exp,
    log,
    log2,
    log10,
    hypot,
    cbrt,
    fmin,
    fmax,
    rand_float,
    rand_range,
    pi,
    e,
    deg_to_rad,
    rad_to_deg,
    is_nan,
    is_infinite,
    tau,
} from "std/math";
import { format_float } from "std/string";

function f2(v: float): string {
    return format_float(v, 4);
}

function main(): int {
    println(`abs=${abs(-5)} fabs=${f2(fabs(-2.5))} min=${min(3, 8)} max=${max(3, 8)}`);
    println(`floor=${f2(floor(2.7))} ceil=${f2(ceil(2.1))} round=${f2(round(2.5))} trunc=${f2(trunc(2.7))}`);
    println(`sqrt(16)=${f2(sqrt(16.0))} cbrt(27)=${f2(cbrt(27.0))} hypot(3,4)=${f2(hypot(3.0, 4.0))}`);
    println(`sign(-3)=${f2(sign(-3.0))} sign(0)=${f2(sign(0.0))} sign(3)=${f2(sign(3.0))}`);
    println(`clamp(50,0,10)=${clamp(50, 0, 10)} gcd(12,18)=${gcd(12, 18)} lcm(4,6)=${lcm(4, 6)}`);
    println(`rand_int(100)=${rand_int(100)} rand_float=${f2(rand_float())} rand_range(1,5)=${f2(rand_range(1.0, 5.0))}`);
    println(`sin(0)=${f2(sin(0.0))} cos(0)=${f2(cos(0.0))} tan(0)=${f2(tan(0.0))}`);
    println(`asin(0)=${f2(asin(0.0))} acos(1)=${f2(acos(1.0))} atan(0)=${f2(atan(0.0))} atan2(1,1)=${f2(atan2(1.0, 1.0))}`);
    println(`exp(0)=${f2(exp(0.0))} log(1)=${f2(log(1.0))} log2(8)=${f2(log2(8.0))} log10(100)=${f2(log10(100.0))}`);
    println(`fmin(1,2)=${f2(fmin(1.0, 2.0))} fmax(1,2)=${f2(fmax(1.0, 2.0))}`);
    println(`pi=${f2(pi())} e=${f2(e())} tau=${f2(tau())} deg_to_rad(180)=${f2(deg_to_rad(180.0))} rad_to_deg(pi)=${f2(rad_to_deg(pi()))}`);
    println(`is_nan(sqrt(-1))=${is_nan(sqrt(-1.0))} is_infinite(1/0)?`);
    return 0;
}
