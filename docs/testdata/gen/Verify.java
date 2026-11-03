import java.io.BufferedInputStream;
import java.io.FileInputStream;
import java.io.ObjectInputStream;
import java.lang.reflect.Field;
import java.util.Map;

/**
 * Reads a Java serialization stream produced by Rust and asserts a class name
 * plus field/entry values.
 *
 * Usage: java -cp build/classes Verify <file.ser> <class-name> [name=value ...]
 * For Map instances the values are looked up with {@code map.get(name)};
 * otherwise the named field is read through reflection.
 */
public final class Verify {
    private Verify() {}

    public static void main(String[] args) throws Exception {
        if (args.length < 2) {
            System.err.println("usage: Verify <file.ser> <class-name> [name=value ...]");
            System.exit(2);
        }

        Object object;
        try (ObjectInputStream in =
                new ObjectInputStream(new BufferedInputStream(new FileInputStream(args[0])))) {
            object = in.readObject();
        }

        String actualClass = object.getClass().getName();
        if (!actualClass.equals(args[1])) {
            throw new AssertionError("class mismatch: expected " + args[1] + ", got " + actualClass);
        }

        for (int i = 2; i < args.length; i++) {
            String spec = args[i];
            int equals = spec.indexOf('=');
            if (equals < 0) {
                throw new IllegalArgumentException("expected name=value, got " + spec);
            }
            String name = spec.substring(0, equals);
            String expected = spec.substring(equals + 1);

            Object actual;
            if (object instanceof Map<?, ?> map) {
                actual = map.get(name);
            } else if (object instanceof java.util.List<?> list) {
                actual = list.get(Integer.parseInt(name));
            } else {
                Field field = object.getClass().getDeclaredField(name);
                field.setAccessible(true);
                actual = field.get(object);
            }

            if (actual == null) {
                throw new AssertionError("missing value for `" + name + "`");
            }
            String text = String.valueOf(actual);
            if (!text.equals(expected)) {
                throw new AssertionError(
                        "`" + name + "` mismatch: expected " + expected + ", got " + text);
            }
        }

        System.out.println("OK " + actualClass);
    }
}
