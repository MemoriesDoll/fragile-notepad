package sample.outline

class Service {
    constructor()

    fun load() {
        fun local() {
        }
    }

    private suspend fun save(value: String): String {
        return value.trim()
    }

    object Registry {
        fun register() {
        }
    }
}

interface Store {
    fun required(value: String)

    fun provided(): String {
        return "store"
    }
}
