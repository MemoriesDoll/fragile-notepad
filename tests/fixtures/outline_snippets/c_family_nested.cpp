int top() {
    return 1;
}

std::string label() {
    return "sample";
}

namespace sample {
class Service {
public:
    Service();
    ~Service();

    void load() {
        if (ready()) {
            run();
        }
    }

    bool ready() const;

    auto count() -> size_t;

    bool operator==(const Service& other) const;

    void operator()(int value) {
    }

    struct Worker {
        int run() {
            return 0;
        }
    };
};

Service::Service() {
}

Service::~Service() {
}

bool Service::ready() const {
    return true;
}

auto Service::count() -> size_t {
    return 2;
}

bool operator==(const Service& left, const Service& right) {
    return left.ready() == right.ready();
}
}
