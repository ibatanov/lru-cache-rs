# LRU Cache Rust Implementation

Потокобезопасный кеш с политикой вытеснения Least Recently Used (LRU), поддержкой TTL и ограничениями по размеру.

## Установка

Добавьте в `Cargo.toml`:

```toml
[dependencies]
lru-cache-rs = { path = "./lru-cache-rs" }
```

## Использование

### Создание кеша

```rust
use lru_cache_rs::SafeLRUCache;
use std::time::Duration;

// Создать кеш на 100 элементов с максимальным размером 1MB
let cache = SafeLRUCache::new(100, Some(1024 * 1024));
```

### Основные операции

**Добавление элемента:**
```rust
// Без TTL
cache.put("key1", "value1", None, 10);

// С TTL (5 секунд)
cache.put("key2", "value2", Some(Duration::from_secs(5)), 20);
```

**Получение элемента:**
```rust
if let Some(value) = cache.get(&"key1") {
    println!("Found: {}", value);
}
```

**Очистка просроченных элементов:**
```rust
cache.clear_expired();
```

## Параметры

- `capacity`: максимальное количество элементов
- `max_size`: максимальный общий размер в байтах (опционально)
- `ttl`: время жизни элемента (опционально)
- `size`: размер элемента в байтах (используется для ограничения по размеру)

## Особенности

- Автоматическое вытеснение старых элементов при достижении лимитов
- Потокобезопасность (можно использовать из нескольких потоков)
- Поддержка времени жизни элементов (TTL)
- Два критерия вытеснения: по количеству и по размеру

## Пример

```rust
use std::thread;

let cache = SafeLRUCache::new(2, None);

cache.put("a", 1, None, 1);
cache.put("b", 2, None, 1);

assert_eq!(cache.get(&"a"), Some(1));

// Добавление нового элемента вытеснит "b" (по принципу LRU)
cache.put("c", 3, None, 1);
assert_eq!(cache.get(&"b"), None);
```
