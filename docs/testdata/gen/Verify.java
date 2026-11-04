import java.io.BufferedInputStream;
import java.io.EOFException;
import java.io.FileInputStream;
import java.io.ObjectInputStream;
import java.io.OptionalDataException;
import java.lang.reflect.Field;
import java.util.Map;

/**
 * Reads a Java serialization stream produced by Rust.
 *
 * Usage:
 *   java Verify <file.ser> <class-name> [name=value ...]
 *       one object; Map/List entries or fields are checked.
 *   java Verify --read <file.ser>
 *       read every top-level object until EOF (handles TC_RESET/shared refs).
 *   java Verify --sequence <file.ser> <class>...
 *       read exactly one object per class and compare class names.
 *   java Verify --primitives <file.ser> <spec>...
 *       read raw primitives, e.g. i=1 z=true d=1.5.
 *   java Verify --blocks <file.ser> <byte-count>
 *       consume top-level block data with ObjectInputStream.readFully.
 *   java Verify --array <file.ser> <array-class> <element-class>...
 *       read an Object[] and check its class, length and element classes.
 */
public final class Verify {
    private Verify() {}

    public static void main(String[] args) throws Exception {
        if (args.length == 0) {
            usage();
        }
        if (args[0].startsWith("--")) {
            switch (args[0]) {
                case "--read" -> readAll(args);
                case "--sequence" -> sequence(args);
                case "--primitives" -> primitives(args);
                case "--blocks" -> blocks(args);
                case "--array" -> array(args);
                default -> usage();
            }
            return;
        }
        if (args.length < 2) {
            usage();
        }
        verifyFields(args);
    }

    private static void usage() {
        System.err.println("usage: Verify <file.ser> <class-name> [name=value ...]");
        System.err.println("       Verify --read <file.ser>");
        System.err.println("       Verify --sequence <file.ser> <class>...");
        System.err.println("       Verify --primitives <file.ser> <spec>...");
        System.err.println("       Verify --blocks <file.ser> <byte-count>");
        System.err.println("       Verify --array <file.ser> <array-class> <element-class>...");
        System.exit(2);
    }

    private static ObjectInputStream open(String file) throws Exception {
        return new ObjectInputStream(new BufferedInputStream(new FileInputStream(file)));
    }

    private static void readAll(String[] args) throws Exception {
        if (args.length != 2) {
            usage();
        }
        StringBuilder classes = new StringBuilder();
        int count = 0;
        try (ObjectInputStream in = open(args[1])) {
            while (true) {
                Object object;
                try {
                    object = in.readObject();
                } catch (EOFException end) {
                    break;
                } catch (OptionalDataException primitive) {
                    throw new AssertionError(
                            "top-level primitive data in " + args[1]
                                    + "; use --primitives/--blocks");
                }
                count++;
                classes.append(object == null ? "null" : object.getClass().getName()).append(' ');
            }
        }
        if (count == 0) {
            throw new AssertionError("stream has no objects: " + args[1]);
        }
        System.out.println("OK " + count + " object(s): " + classes.toString().trim());
    }

    private static void sequence(String[] args) throws Exception {
        if (args.length < 3) {
            usage();
        }
        try (ObjectInputStream in = open(args[1])) {
            for (int i = 2; i < args.length; i++) {
                Object object;
                try {
                    object = in.readObject();
                } catch (EOFException end) {
                    throw new AssertionError("expected more objects: " + args[1]);
                }
                String actual = object == null ? "null" : object.getClass().getName();
                if (!actual.equals(args[i])) {
                    throw new AssertionError(
                            "object " + (i - 2) + ": expected " + args[i] + ", got " + actual);
                }
            }
        }
        System.out.println("OK sequence of " + (args.length - 2));
    }

    private static void primitives(String[] args) throws Exception {
        if (args.length < 2) {
            usage();
        }
        try (ObjectInputStream in = open(args[1])) {
            for (int i = 2; i < args.length; i++) {
                String spec = args[i];
                int equals = spec.indexOf('=');
                if (equals < 0) {
                    throw new IllegalArgumentException("expected name=value, got " + spec);
                }
                String name = spec.substring(0, equals);
                String expected = spec.substring(equals + 1);
                Object actual = switch (name.charAt(0)) {
                    case 'i' -> in.readInt();
                    case 'z' -> in.readBoolean();
                    case 'd' -> in.readDouble();
                    case 'l' -> in.readLong();
                    case 'f' -> in.readFloat();
                    case 's' -> in.readShort();
                    case 'b' -> in.readByte();
                    case 'c' -> in.readChar();
                    default -> throw new IllegalArgumentException("unknown primitive " + name);
                };
                if (!String.valueOf(actual).equals(expected)) {
                    throw new AssertionError(
                            name + " mismatch: expected " + expected + ", got " + actual);
                }
            }
        }
        System.out.println("OK primitives");
    }

    private static void blocks(String[] args) throws Exception {
        if (args.length != 3) {
            usage();
        }
        int length = Integer.parseInt(args[2]);
        try (ObjectInputStream in = open(args[1])) {
            byte[] bytes = new byte[length];
            in.readFully(bytes);
        }
        System.out.println("OK blocks " + length);
    }

    private static void array(String[] args) throws Exception {
        if (args.length < 3) {
            usage();
        }
        Object[] array;
        try (ObjectInputStream in = open(args[1])) {
            array = (Object[]) in.readObject();
        }
        String actualClass = array.getClass().getName();
        if (!actualClass.equals(args[2])) {
            throw new AssertionError("array class mismatch: expected " + args[2] + ", got " + actualClass);
        }
        int expectedLength = args.length - 3;
        if (array.length != expectedLength) {
            throw new AssertionError(
                    "array length mismatch: expected " + expectedLength + ", got " + array.length);
        }
        for (int i = 0; i < array.length; i++) {
            String actual = array[i] == null ? "null" : array[i].getClass().getName();
            if (!actual.equals(args[3 + i])) {
                throw new AssertionError(
                        "array[" + i + "]: expected " + args[3 + i] + ", got " + actual);
            }
        }
        System.out.println("OK array " + actualClass + "[" + array.length + "]");
    }

    private static void verifyFields(String[] args) throws Exception {
        Object object;
        try (ObjectInputStream in = open(args[0])) {
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
