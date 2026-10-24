package okkama.rt;

import java.math.BigInteger;
import java.util.regex.Pattern;

/**
 * Natives {@code runtime/bigint} (provider {@code core.bigint}): целые
 * произвольной точности в канонической десятичной записи. Ожидаемые сбои —
 * {@link NativeFailure}; неверная запись операнда — программная ошибка
 * {@code Native invocation "…" failed}, как в Perl-интерпретаторе.
 */
public final class BigIntNatives {
    private BigIntNatives() {}

    private static final String DOMAIN = "core.bigint";
    private static final Pattern DECIMAL = Pattern.compile("-?[0-9]+");

    private static String symbol(String name, int arity) {
        return "runtime/bigint@1::" + name + "/" + arity;
    }

    private static BigInteger number(String origin, String name, int arity) {
        if (!DECIMAL.matcher(origin).matches()) {
            throw Rt.trap("Native invocation \"" + symbol(name, arity) + "\" failed");
        }
        return new BigInteger(origin);
    }

    private static Failure failure(String code, String message, String name, int arity) {
        return NativeFailure.raise(DOMAIN, code, symbol(name, arity), message);
    }

    public static boolean runtime_bigint__bigint_valid(String origin) {
        return DECIMAL.matcher(origin).matches();
    }

    public static String runtime_bigint__bigint_canonical(String origin) {
        return number(origin, "bigint_canonical", 1).toString();
    }

    public static BigInteger runtime_bigint__bigint_sign(String origin) {
        return BigInteger.valueOf(number(origin, "bigint_sign", 1).signum());
    }

    public static BigInteger runtime_bigint__bigint_bits(String origin) {
        return BigInteger.valueOf(number(origin, "bigint_bits", 1).abs().bitLength());
    }

    public static String runtime_bigint__bigint_difference(String left, String right) {
        return number(left, "bigint_difference", 2)
            .subtract(number(right, "bigint_difference", 2)).toString();
    }

    public static BigInteger runtime_bigint__bigint_order(String left, String right) {
        return BigInteger.valueOf(
            number(left, "bigint_order", 2).compareTo(number(right, "bigint_order", 2)));
    }

    public static String runtime_bigint__bigint_sum(String left, String right) {
        return number(left, "bigint_sum", 2).add(number(right, "bigint_sum", 2)).toString();
    }

    public static String runtime_bigint__bigint_product(String left, String right) {
        return number(left, "bigint_product", 2)
            .multiply(number(right, "bigint_product", 2)).toString();
    }

    /** Частное усекается к нулю, остаток — со знаком делимого. */
    public static String runtime_bigint__bigint_quotient(String left, String right) {
        return division(left, right, false, "bigint_quotient");
    }

    public static String runtime_bigint__bigint_remainder(String left, String right) {
        return division(left, right, true, "bigint_remainder");
    }

    private static String division(String leftOrigin, String rightOrigin, boolean remainder, String name) {
        BigInteger left = number(leftOrigin, name, 2);
        BigInteger right = number(rightOrigin, name, 2);
        if (right.signum() == 0) {
            throw failure("invalid_input", "BigInt division by zero", name, 2);
        }
        return (remainder ? left.remainder(right) : left.divide(right)).toString();
    }

    public static String runtime_bigint__bigint_gcd(String left, String right) {
        return number(left, "bigint_gcd", 2).gcd(number(right, "bigint_gcd", 2)).toString();
    }

    public static String runtime_bigint__bigint_lcm(String leftOrigin, String rightOrigin) {
        BigInteger left = number(leftOrigin, "bigint_lcm", 2).abs();
        BigInteger right = number(rightOrigin, "bigint_lcm", 2).abs();
        if (left.signum() == 0 || right.signum() == 0) {
            return "0";
        }
        return left.divide(left.gcd(right)).multiply(right).toString();
    }

    public static String runtime_bigint__bigint_power(String origin, BigInteger exponent, BigInteger maximumBits) {
        String name = "bigint_power";
        if (exponent.signum() < 0) {
            throw failure("invalid_input", "BigInt exponent must be non-negative", name, 3);
        }
        if (maximumBits.signum() <= 0) {
            throw failure("invalid_input", "BigInt power bit limit must be positive", name, 3);
        }
        BigInteger base = number(origin, name, 3);
        if (exponent.signum() == 0) {
            return "1";
        }
        if (base.signum() == 0) {
            return "0";
        }
        BigInteger absolute = base.abs();
        if (absolute.equals(BigInteger.ONE)) {
            return base.signum() < 0 && exponent.testBit(0) ? "-1" : "1";
        }
        // У k-битного |base| >= 2 в результате не меньше (k-1)*exponent+1 бит.
        BigInteger lowerBound = exponent
            .multiply(BigInteger.valueOf(absolute.bitLength() - 1)).add(BigInteger.ONE);
        if (lowerBound.compareTo(maximumBits) > 0 || exponent.bitLength() > 31) {
            throw failure("resource_exhausted", "BigInt power exceeds bit limit", name, 3);
        }
        BigInteger result = base.pow(exponent.intValue());
        if (BigInteger.valueOf(result.abs().bitLength()).compareTo(maximumBits) > 0) {
            throw failure("resource_exhausted", "BigInt power exceeds bit limit", name, 3);
        }
        return result.toString();
    }
}
