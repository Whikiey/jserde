import java.io.FileOutputStream;
import java.io.ObjectInputStream;
import java.io.ObjectOutputStream;
import java.io.ObjectStreamClass;
import java.io.Serializable;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.BitSet;
import java.util.Collections;
import java.util.Currency;
import java.util.Date;
import java.util.EnumMap;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Hashtable;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.LinkedList;
import java.util.List;
import java.util.Locale;
import java.util.PriorityQueue;
import java.util.Properties;
import java.util.Stack;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.UUID;
import java.util.Vector;
import java.util.regex.Pattern;
import java.time.Duration;
import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.MonthDay;
import java.time.OffsetDateTime;
import java.time.OffsetTime;
import java.time.Period;
import java.time.Year;
import java.time.YearMonth;
import java.time.ZoneId;
import java.time.ZoneOffset;
import java.time.ZonedDateTime;

/**
 * 基准数据生成器（权威来源）。
 *
 *   javac -d build/classes gen/Fixtures.java
 *   java -cp build/classes Fixtures fixtures/codec fixtures/object
 *
 * 参数 1 写 codec 层用例（无类描述符的流原语），参数 2 写 object 层用例（含
 * 类描述符、SUID、集合自定义布局）。未给出的目录不生成。
 *
 * stdout 输出 SUID 表（供 Rust 侧对照 ObjectStreamClass.computeDefaultSUID 实现），
 * stderr 输出进度。
 *
 * 约定：测试类显式声明 serialVersionUID = 1L，保证字节跨 JDK 版本稳定；
 * 唯一例外是 NoSuid，它的 SUID 是 JDK 计算出来的，用来校验 Rust 侧算法。
 */
public class Fixtures {

    interface Body {
        void write(ObjectOutputStream out) throws Exception;
    }

    // ================= 测试用类 =================

    static class Point implements Serializable {
        private static final long serialVersionUID = 1L;
        final int x;
        final int y;

        Point(int x, int y) {
            this.x = x;
            this.y = y;
        }
    }

    /** 基本类型与对象字段混排：用于核对字段顺序与块缓冲的 flush 时机。 */
    static class Mixed implements Serializable {
        private static final long serialVersionUID = 1L;
        int i = 7;
        long l = 8L;
        String s = "mix";
        int[] a = {1, 2};
        boolean b = true;
        double d = 1.5;
        float f = 0.5f;
        char c = '中';
        short sh = -3;
        byte by = 9;
    }

    static class Base implements Serializable {
        private static final long serialVersionUID = 1L;
        int a = 1;
    }

    static class Derived extends Base {
        private static final long serialVersionUID = 1L;
        String b = "b";
    }

    /** SC_WRITE_METHOD：字段之后还跟着自定义数据，以 TC_ENDBLOCKDATA 收尾。 */
    static class CustomWritable implements Serializable {
        private static final long serialVersionUID = 1L;
        int value = 42;

        private void writeObject(ObjectOutputStream out) throws java.io.IOException {
            out.defaultWriteObject();
            out.writeInt(1234);
            out.writeUTF("custom");
        }

        private void readObject(ObjectInputStream in)
                throws java.io.IOException, ClassNotFoundException {
            in.defaultReadObject();
            in.readInt();
            in.readUTF();
        }
    }

    enum Color { RED, GREEN }

    static class Node implements Serializable {
        private static final long serialVersionUID = 1L;
        String name;
        Node next;

        Node(String name) {
            this.name = name;
        }
    }

    static class WithTransient implements Serializable {
        private static final long serialVersionUID = 1L;
        int a = 1;
        transient int t = 99;
        String s = "s";
    }

    /** 不声明 SUID：暴露 JDK 计算出的默认值，用于校验我们的算法。 */
    static class NoSuid implements Serializable {
        int x = 1;
        String y = "y";
    }

    /** 三层对象嵌套：Outer -> Middle -> Leaf。 */
    static class Leaf implements Serializable {
        private static final long serialVersionUID = 1L;
        int value;

        Leaf(int value) {
            this.value = value;
        }
    }

    static class Middle implements Serializable {
        private static final long serialVersionUID = 1L;
        String name;
        Leaf leaf;

        Middle(String name, Leaf leaf) {
            this.name = name;
            this.leaf = leaf;
        }
    }

    static class Outer implements Serializable {
        private static final long serialVersionUID = 1L;
        Middle middle;

        Outer(Middle middle) {
            this.middle = middle;
        }
    }

    /** 混合嵌套：Map/数组/自定义类同图，且共享同一个 Point 并自引用成环。 */
    static class Hybrid implements Serializable {
        private static final long serialVersionUID = 1L;
        Object[] array;
        HashMap<String, Object> map = new HashMap<>();
        Hybrid self;
        Point shared;
    }

    // ================= 生成逻辑 =================

    static int count = 0;

    static void gen(Path dir, String file, Body body) throws Exception {
        Path target = dir.resolve(file);
        try (ObjectOutputStream out = new ObjectOutputStream(new FileOutputStream(target.toFile()))) {
            body.write(out);
        }
        count++;
        System.err.printf("  %-40s %8d bytes%n", file, Files.size(target));
    }

    static void codec(Path dir) throws Exception {
        System.err.println("[codec] " + dir);
        gen(dir, "01-string-ascii.ser", out -> out.writeObject("hello"));
        gen(dir, "02-string-empty.ser", out -> out.writeObject(""));
        gen(dir, "03-string-nul.ser", out -> out.writeObject("a\u0000b"));
        gen(dir, "04-string-cjk.ser", out -> out.writeObject("中文"));
        gen(dir, "05-string-surrogate-pair.ser", out -> out.writeObject("😀"));
        gen(dir, "06-string-long.ser", out -> out.writeObject("x".repeat(70000)));
        gen(dir, "07-null.ser", out -> out.writeObject(null));
        gen(dir, "08-string-shared-ref.ser", out -> {
            String s = "ab";
            out.writeObject(s);
            out.writeObject(s);
        });
        gen(dir, "09-primitives-block.ser", out -> {
            out.writeInt(1);
            out.writeBoolean(true);
            out.writeDouble(1.5);
        });
        gen(dir, "10-reset.ser", out -> {
            out.writeObject("a");
            out.reset();
            out.writeObject("b");
        });
        gen(dir, "11-blockdata-long.ser", out -> {
            byte[] payload = new byte[2000];
            for (int i = 0; i < payload.length; i++) {
                payload[i] = (byte) (i & 0xFF);
            }
            out.write(payload);
        });
        gen(dir, "12-utf-widths.ser", out -> out.writeObject("a\u00e9\u4e2d"));
    }

    static void objects(Path dir) throws Exception {
        System.err.println("[object] " + dir);

        gen(dir, "20-object-point.ser", out -> out.writeObject(new Point(1, 2)));

        gen(dir, "21-object-mixed-fields.ser", out -> out.writeObject(new Mixed()));

        gen(dir, "22-object-inheritance.ser", out -> out.writeObject(new Derived()));

        gen(dir, "23-object-custom-write-method.ser", out -> out.writeObject(new CustomWritable()));

        gen(dir, "24-object-enum.ser", out -> out.writeObject(Color.GREEN));

        gen(dir, "25-array-int.ser", out -> out.writeObject(new int[] {1, 2, 3}));

        gen(dir, "26-array-string-null.ser", out -> out.writeObject(new String[] {"a", "b", null}));

        gen(dir, "27-array-2d.ser", out -> out.writeObject(new int[][] {{1, 2}, {3}}));

        gen(dir, "28-collection-arraylist.ser", out -> {
            List<String> list = new ArrayList<>();
            list.add("alpha");
            list.add("beta");
            out.writeObject(list);
        });

        gen(dir, "29-collection-hashmap.ser", out -> {
            HashMap<String, Integer> map = new HashMap<>();
            map.put("one", 1);
            map.put("two", 2);
            map.put("three", 3);
            out.writeObject(map);
        });

        gen(dir, "30-collection-linkedhashmap.ser", out -> {
            LinkedHashMap<String, Integer> map = new LinkedHashMap<>();
            map.put("a", 1);
            map.put("b", 2);
            out.writeObject(map);
        });

        gen(dir, "31-collection-hashset.ser", out -> {
            HashSet<String> set = new HashSet<>();
            set.add("x");
            set.add("y");
            out.writeObject(set);
        });

        gen(dir, "32-collection-linkedlist.ser", out -> {
            LinkedList<Integer> list = new LinkedList<>();
            list.add(1);
            list.add(2);
            list.add(3);
            out.writeObject(list);
        });

        gen(dir, "33-ref-shared-top.ser", out -> {
            Point p = new Point(5, 6);
            out.writeObject(p);
            out.writeObject(p);
        });

        gen(dir, "34-ref-cycle.ser", out -> {
            Node a = new Node("a");
            Node b = new Node("b");
            a.next = b;
            b.next = a;
            out.writeObject(a);
        });

        gen(dir, "35-reset-mid-stream.ser", out -> {
            out.writeObject(new Point(1, 2));
            out.reset();
            out.writeObject(new Point(3, 4));
        });

        gen(dir, "36-misc-java-types.ser", out -> out.writeObject(new Object[] {
                new Date(0L),
                new BigInteger("123456789012345678901234567890"),
                new BigDecimal("1.50"),
                LocalDate.of(2024, 2, 29),
        }));

        gen(dir, "37-object-with-transient.ser", out -> out.writeObject(new WithTransient()));

        gen(dir, "38-collection-nested-map.ser", out -> {
            HashMap<String, HashMap<String, Integer>> outer = new HashMap<>();
            HashMap<String, Integer> inner = new HashMap<>();
            inner.put("k", 7);
            outer.put("outer", inner);
            out.writeObject(outer);
        });

        gen(dir, "39-suid-computed.ser", out -> out.writeObject(new NoSuid()));

        gen(dir, "40-java-time.ser", out -> out.writeObject(new Object[] {
                Duration.ofSeconds(123, 456789000),
                Instant.ofEpochSecond(1700000000, 123456789),
                LocalDate.of(2024, 2, 29),
                LocalDateTime.of(2024, 2, 29, 12, 34, 56, 123456789),
                ZonedDateTime.of(2024, 2, 29, 12, 34, 56, 123456789,
                        ZoneId.of("Asia/Shanghai")),
        }));

        gen(dir, "41-nested-objects.ser", out -> out.writeObject(
                new Outer(new Middle("mid", new Leaf(42)))));

        gen(dir, "42-java-time-more.ser", out -> out.writeObject(new Object[] {
                Year.of(2024),
                YearMonth.of(2024, 2),
                MonthDay.of(2, 29),
                Period.of(1, 2, 3),
        }));

        gen(dir, "43-java-time-zone.ser", out -> out.writeObject(new Object[] {
                ZoneId.of("Asia/Shanghai"),
                ZoneOffset.ofHours(8),
                OffsetTime.of(12, 34, 56, 123456789, ZoneOffset.ofHours(8)),
                OffsetDateTime.of(2024, 2, 29, 12, 34, 56, 123456789, ZoneOffset.ofHours(8)),
        }));

        gen(dir, "44-hybrid-nested.ser", out -> {
            Point shared = new Point(7, 8);
            Hybrid hybrid = new Hybrid();
            hybrid.map.put("point", shared);
            hybrid.map.put("ints", new int[] {1, 2, 3});
            hybrid.map.put("text", "mix");
            hybrid.map.put("list", new ArrayList<>(List.of("a", "b")));
            hybrid.array = new Object[] {shared, new int[] {4, 5}, "tail"};
            hybrid.shared = shared;
            hybrid.self = hybrid;
            out.writeObject(hybrid);
        });

        gen(dir, "45-value-classes.ser", out -> out.writeObject(new Object[] {
                new UUID(0x0123456789abcdefL, 0xfedcba9876543210L),
                Currency.getInstance("USD"),
                Locale.US,
                BitSet.valueOf(new long[] {0b1010L, 0xFFFF_FFFFL}),
                new StringBuilder("builder"),
                new StringBuffer("buffer"),
                Pattern.compile("a+b"),
                new java.sql.Date(0L),
                new java.sql.Time(0L),
                java.sql.Timestamp.from(Instant.ofEpochSecond(0, 123456789)),
        }));

        gen(dir, "46-containers.ser", out -> {
            LinkedHashSet<String> linkedSet = new LinkedHashSet<>(List.of("a", "b"));
            TreeMap<String, Integer> treeMap = new TreeMap<>();
            treeMap.put("a", 1);
            treeMap.put("b", 2);
            TreeSet<String> treeSet = new TreeSet<>(List.of("x", "y"));
            ArrayDeque<String> deque = new ArrayDeque<>(List.of("p", "q"));
            PriorityQueue<Integer> queue = new PriorityQueue<>(List.of(3, 1, 2));
            Vector<String> vector = new Vector<>(List.of("v1", "v2"));
            Stack<String> stack = new Stack<>();
            stack.push("s1");
            stack.push("s2");
            Hashtable<String, Integer> table = new Hashtable<>();
            table.put("h1", 1);
            table.put("h2", 2);
            Properties properties = new Properties();
            properties.setProperty("k", "v");
            List<String> unmodifiable = Collections.unmodifiableList(new ArrayList<>(List.of("u1", "u2")));
            List<String> synchronizedList = Collections.synchronizedList(new ArrayList<>(List.of("s1", "s2")));
            List<String> arraysList = Arrays.asList("a1", "a2");

            out.writeObject(new Object[] {
                    linkedSet, treeMap, treeSet, deque, queue, vector, stack,
                    table, properties, unmodifiable, synchronizedList, arraysList,
            });
        });

        gen(dir, "47-class-objects.ser", out -> out.writeObject(new Object[] {
                String.class, String.class, Color.class, Mixed.class,
        }));
    }

    static void printSuids() {
        Class<?>[] classes = {
                Point.class, Mixed.class, Base.class, Derived.class, CustomWritable.class,
                Color.class, Node.class, WithTransient.class, NoSuid.class,
                Leaf.class, Middle.class, Outer.class,
        };
        System.out.println("# name\tsuid");
        for (Class<?> c : classes) {
            long suid = ObjectStreamClass.lookup(c).getSerialVersionUID();
            System.out.println(c.getName() + "\t" + suid);
        }
    }

    public static void main(String[] args) throws Exception {
        if (args.length == 0) {
            System.err.println("用法: java Fixtures <codec 目录> [object 目录]");
            System.exit(2);
        }
        Path codecDir = Path.of(args[0]);
        Files.createDirectories(codecDir);
        codec(codecDir);
        if (args.length > 1) {
            Path objDir = Path.of(args[1]);
            Files.createDirectories(objDir);
            objects(objDir);
        }
        printSuids();
        System.err.println("共生成 " + count + " 个文件");
    }
}
