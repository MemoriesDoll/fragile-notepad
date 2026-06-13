package sample.outline;

public class Service {
    public Service() {
    }

    public void load() {
        class LocalJob {
            void run() {
            }
        }
    }

    private static String format(String value) {
        return value.trim();
    }

    interface Store {
        void save(String value);

        default String name() {
            return "store";
        }
    }
}
