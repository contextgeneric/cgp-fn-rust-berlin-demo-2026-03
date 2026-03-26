# cgp-example-profile-picture

Welcome to `cgp-example-profile-picture`, a step-by-step tutorial that introduces
[Context-Generic Programming](https://contextgeneric.dev) (CGP) through a practical,
real-world feature: fetching a user's profile picture from a cloud storage service.

This repository was originally created as a live demonstration at the Rust Berlin meetup
in March 2026, and has since been expanded into a self-contained tutorial that you can
work through at your own pace. No prior knowledge of CGP is required — all you need is a
basic familiarity with Rust, including functions, structs, traits, and `async`/`await`.

## What You Will Learn

This tutorial takes a single feature — retrieving a user's profile picture — and
implements it in five progressively more capable ways. Each step exposes a limitation
in the previous approach, and introduces a CGP tool that addresses it. By the end, you
will understand why plain Rust code can become rigid and repetitive as applications grow,
and how CGP provides a principled, incremental solution.

The tutorial covers the following stages:

1. **Plain functions** — the simplest possible starting point, with all dependencies
   passed explicitly as function arguments.
2. **Methods on a concrete context** — bundling dependencies into a struct to clean up
   call signatures, and the new problems that creates.
3. **Context-generic functions with `#[cgp_fn]`** — writing a single function definition
   that works automatically for any context containing the required fields.
4. **Generic backends with `#[impl_generics]`** — extending context-generic functions to
   work across different database backends using a generic type parameter.
5. **Multiple implementations with `#[cgp_impl]`** — supporting alternative
   implementations selected at compile time, enabling different storage backends for
   different deployment contexts.

## The Running Example

The feature we will implement throughout this tutorial is `get_user_profile_picture`: a
function that retrieves a user's profile picture from a cloud storage bucket. The function
has three responsibilities.

First, it must look up the user's record from a database using the supplied user identifier.
Second, it must check whether the user record contains a stored reference to a profile
picture object. Third, if such a reference exists, it must download the image bytes from
a cloud storage bucket and decode them into an image value.

This feature is deliberately chosen because it requires multiple infrastructure
dependencies — a database connection, a cloud storage client, and a bucket identifier —
which makes dependency management problems highly visible even at this small scale.

All the shared types used throughout the tutorial are defined in `src/types.rs`:

```rust
pub struct UserId(pub u64);

#[derive(sqlx::FromRow)]
pub struct User {
    pub name: String,
    pub email: String,
    pub profile_picture_object_id: Option<String>,
}
```

`UserId` is a newtype wrapper around a plain `u64`. The `User` struct represents a
database row: it carries the user's name and email, and an optional string that identifies
the user's profile picture object in cloud storage. The `#[derive(sqlx::FromRow)]`
annotation tells [sqlx](https://github.com/launchbadge/sqlx) how to automatically
deserialize database rows into `User` values.

The tutorial is structured as five chapters, each corresponding to a source file under
src:

| Chapter | Source file            | Topic                                      |
|---------|------------------------|--------------------------------------------|
| 1       | plain_fn.rs      | Plain async functions                      |
| 2       | method.rs        | Methods on a concrete `App` struct         |
| 3       | cgp_fn.rs        | Context-generic functions with `#[cgp_fn]` |
| 4       | cgp_fn_generic.rs| Generic database backends                  |
| 5       | cgp_impl.rs      | Multiple implementations with `#[cgp_impl]`|

The application contexts referenced throughout the tutorial are defined under
contexts. Each context is a struct that bundles a specific combination of
infrastructure dependencies. You will see which contexts gain and lose capabilities as
the implementation evolves.

---

## Chapter 1: Plain Functions

*Source file: plain_fn.rs*

### The Most Natural Starting Point

The most straightforward way to implement `get_user_profile_picture` in Rust is as a
set of plain async functions. There are no traits, no generics, and no abstraction layers
— just functions that accept their dependencies directly as arguments and return results.

The first helper retrieves a user from the database:

```rust
pub async fn get_user(database: &PgPool, user_id: &UserId) -> anyhow::Result<User> {
    let user =
        sqlx::query_as("SELECT name, email, profile_picture_object_id FROM users WHERE id = $1")
            .bind(user_id.0 as i64)
            .fetch_one(database)
            .await?;

    Ok(user)
}
```

`get_user` takes two parameters: a reference to the PostgreSQL connection pool and the
user identifier to look up. It issues a parameterized SQL query using `sqlx` and returns
the deserialized `User` row, or an error wrapped in `anyhow::Result`.

The second helper downloads raw bytes from a cloud storage bucket:

```rust
pub async fn fetch_storage_object(
    storage_client: &Client,
    bucket_id: &str,
    object_id: &str,
) -> anyhow::Result<Vec<u8>> {
    let output = storage_client
        .get_object()
        .bucket(bucket_id)
        .key(object_id)
        .send()
        .await?;

    let data = output.body.collect().await?.into_bytes().to_vec();
    Ok(data)
}
```

`fetch_storage_object` requires three separate parameters: the AWS S3 client, the name
of the bucket, and the object key to download. It returns the raw bytes of the stored
object.

The top-level function combines both helpers:

```rust
pub async fn get_user_profile_picture(
    database: &PgPool,
    storage_client: &Client,
    bucket_id: &str,
    user_id: &UserId,
) -> anyhow::Result<Option<RgbImage>> {
    let user = get_user(database, user_id).await?;

    if let Some(object_id) = user.profile_picture_object_id {
        let data = fetch_storage_object(storage_client, bucket_id, &object_id).await?;

        let image = image::load_from_memory(&data)?.to_rgb8();

        Ok(Some(image))
    } else {
        Ok(None)
    }
}
```

`get_user_profile_picture` calls both helpers and combines their results. If the user
does not have a stored profile picture identifier, it returns `None`. Otherwise, it
downloads the object and decodes the bytes into an RGB image using the `image` crate.

### The Problem: Dependency Forwarding

This code is easy to read and there is nothing technically wrong with it for a small
program. However, the approach quickly reveals a fundamental limitation as the application
grows.

`get_user_profile_picture` does not directly interact with the database, the storage
client, or the bucket name. Its only genuine input is the `user_id`. Yet its signature
must list all four parameters, because it is responsible for forwarding three of them to
its callees. If `get_user` were later refactored to also require, say, a metrics client
or a cache connection, the parameter list of `get_user_profile_picture` would need to
grow accordingly — even though it still does not use those additional dependencies itself.

This **dependency forwarding** problem compounds at every level of the call chain. Any
code that calls `get_user_profile_picture` must also hold references to all of its
parameters. A top-level HTTP request handler, for example, would need to carry and thread
every single dependency that is transitively required by every function it eventually
calls. As the application grows to dozens or hundreds of such functions, call sites become
progressively harder to read and refactor.

A second limitation is that the functions are hardcoded to specific concrete types. The
database must be a `PgPool` (PostgreSQL) and the storage client must be an
`aws_sdk_s3::Client`. There is no mechanism to substitute a different database backend
for testing, or to support Google Cloud Storage as an alternative to AWS S3, without
rewriting or duplicating the functions.

---

## Chapter 2: Methods on a Concrete Context

*Source file: method.rs*

### Bundling Dependencies Into a Struct

The standard solution in Rust to the dependency-forwarding problem is to group the
dependencies into a single context struct, and expose the operations as methods on that
struct. Instead of threading parameters explicitly through every call, functions can
access what they need directly through `self`.

The `App` context struct is defined in app.rs:

```rust
#[derive(HasField)]
pub struct App {
    pub database: PgPool,
    pub storage_client: Client,
    pub bucket_id: String,
}
```

`App` holds the three infrastructure dependencies required by our feature. The
`#[derive(HasField)]` annotation comes from CGP and generates field-accessor
implementations that will become useful in later chapters. For now, you can think of it
as a harmless annotation that prepares the struct for future use.

With `App` defined, we can rewrite all three functions as `impl App` methods:

```rust
impl App {
    pub async fn get_user(&self, user_id: &UserId) -> anyhow::Result<User> {
        let user = sqlx::query_as(
            "SELECT name, email, profile_picture_object_id FROM users WHERE id = $1",
        )
        .bind(user_id.0 as i64)
        .fetch_one(&self.database)
        .await?;

        Ok(user)
    }

    pub async fn fetch_storage_object(&self, object_id: &str) -> anyhow::Result<Vec<u8>> {
        let output = self
            .storage_client
            .get_object()
            .bucket(&self.bucket_id)
            .key(object_id)
            .send()
            .await?;

        let data = output.body.collect().await?.into_bytes().to_vec();
        Ok(data)
    }

    pub async fn get_user_profile_picture(
        &self,
        user_id: &UserId,
    ) -> anyhow::Result<Option<RgbImage>> {
        let user = self.get_user(user_id).await?;

        if let Some(object_id) = user.profile_picture_object_id {
            let data = self.fetch_storage_object(&object_id).await?;
            let image = image::load_from_memory(&data)?.to_rgb8();

            Ok(Some(image))
        } else {
            Ok(None)
        }
    }
}
```

The improvement is immediately visible. `get_user` now only needs `user_id` as an
explicit parameter; the database connection is accessed silently through `self.database`.
`get_user_profile_picture` also only requires `user_id`, because it can delegate to
`self.get_user` and `self.fetch_storage_object` without knowing which fields are used
internally by those methods. The business logic is no longer obscured by dependency
plumbing.

### The Problem: Tight Coupling to a Single Context

While the method approach solves dependency forwarding, it introduces a different kind of
rigidity: every function is now permanently coupled to the `App` type.

Consider the variety of application contexts that a real project would need. The
contexts directory shows several examples:

```rust
// src/contexts/minimal.rs
pub struct MinimalApp {
    pub database: PgPool,
}

// src/contexts/smart.rs
pub struct SmartApp {
    pub database: PgPool,
    pub storage_client: Client,
    pub bucket_id: String,
    pub open_ai_client: openai::Client,
    pub open_ai_agent: Agent<openai::CompletionModel>,
}

// src/contexts/embedded.rs
pub struct EmbeddedApp {
    pub database: SqlitePool,
    pub storage_client: Client,
    pub bucket_id: String,
}
```

`MinimalApp` is a lean context for services that only need database access — perhaps a
background job processor or a read-only reporting service. `SmartApp` is a richer context
that bundles an OpenAI language model agent alongside the standard infrastructure.
`EmbeddedApp` targets edge deployments or local testing environments where SQLite is
preferred over a full PostgreSQL server.

With the `impl App` approach, none of these contexts can reuse any of the three methods.
If the team building `MinimalApp` needs to look up users from the database, they must
duplicate the `get_user` implementation on their own context. If a developer working on
`EmbeddedApp` wants to use the same `get_user` logic with SQLite, they are out of luck
because the implementation is hardcoded to `PgPool`.

As the application grows to have dozens or hundreds of such functions, this duplication
becomes untenable. Every new context type requires every shared method to be reimplemented
from scratch, and any bug fix or schema change must be applied in multiple places.

What would be ideal is a way to write `get_user` once, and have it automatically work
for *any* context that contains a `database` field of the appropriate type — without the
implementor having to write any per-context glue code. That is exactly what CGP provides.

---

## Chapter 3: Context-Generic Functions with `#[cgp_fn]`

*Source file: cgp_fn.rs*

This is where Context-Generic Programming enters. CGP provides a macro called `#[cgp_fn]`
that transforms a function definition into a *context-generic* implementation. A
context-generic function works automatically for any type that contains the required
fields, regardless of what other fields that type may have. No per-context boilerplate is
needed.

### Introducing `&self` and `#[implicit]` Arguments

Here is the CGP version of `get_user`:

```rust
#[cgp_fn]
#[async_trait]
pub async fn get_user(
    &self,
    #[implicit] database: &PgPool,
    user_id: &UserId,
) -> anyhow::Result<User> {
    let user =
        sqlx::query_as("SELECT name, email, profile_picture_object_id FROM users WHERE id = $1")
            .bind(user_id.0 as i64)
            .fetch_one(database)
            .await?;

    Ok(user)
}
```

The implementation body is completely unchanged from the plain function in Chapter 1.
What has changed is the function signature, and there are three new pieces to notice.

The `#[cgp_fn]` attribute is the primary annotation that activates context-generic
behavior for this function. It instructs the CGP macro system to expand the definition
into the appropriate set of Rust traits and blanket implementations, which are what
actually enable the function to work across different contexts.

The `&self` parameter is new. It represents a reference to a *generic context* — a type
that is not yet known at the point of definition. When you later call `app.get_user(id)`
on a concrete `App` value, Rust resolves `self` to `App`. When you call it on a
`SmartApp`, Rust resolves `self` to `SmartApp`. The `#[cgp_fn]` machinery handles
everything needed to make this work transparently.

The `database` parameter is annotated with `#[implicit]`. This tells `#[cgp_fn]` that
the database value should not be supplied by the caller explicitly. Instead, CGP will
automatically *extract* it from `self` at the call site. Specifically, it looks for a
field named `database` with a value of type `PgPool` in whatever context `self` points
to. As long as the context struct was annotated with `#[derive(HasField)]`, this
extraction is resolved entirely at compile time with no runtime overhead. The `user_id`
parameter has no `#[implicit]` annotation, so it remains an ordinary argument that the
caller must supply explicitly, exactly as before.

The `#[async_trait]` attribute is a standard Rust workaround for using `async fn` in
traits, using the [`async-trait`](https://crates.io/crates/async-trait) crate. It is
required here because `#[cgp_fn]` generates a trait under the hood.

Similarly, `fetch_storage_object` becomes:

```rust
#[cgp_fn]
#[async_trait]
pub async fn fetch_storage_object(
    &self,
    #[implicit] storage_client: &Client,
    #[implicit] bucket_id: &str,
    object_id: &str,
) -> anyhow::Result<Vec<u8>> {
    let output = storage_client
        .get_object()
        .bucket(bucket_id)
        .key(object_id)
        .send()
        .await?;

    let data = output.body.collect().await?.into_bytes().to_vec();
    Ok(data)
}
```

Here, both `storage_client` and `bucket_id` are marked `#[implicit]`. A context only
needs to contain `storage_client: Client` and `bucket_id: String` to automatically gain
the ability to call `self.fetch_storage_object(object_id)`.

### Composing Functions with `#[uses]`

Now that `get_user` and `fetch_storage_object` are context-generic, the top-level
function can call them through `self`:

```rust
#[cgp_fn]
#[async_trait]
#[uses(GetUser, FetchStorageObject)]
pub async fn get_user_profile_picture(&self, user_id: &UserId) -> anyhow::Result<Option<RgbImage>> {
    let user = self.get_user(user_id).await?;

    if let Some(object_id) = user.profile_picture_object_id {
        let data = self.fetch_storage_object(&object_id).await?;
        let image = image::load_from_memory(&data)?.to_rgb8();

        Ok(Some(image))
    } else {
        Ok(None)
    }
}
```

`get_user_profile_picture` does not need any `#[implicit]` parameters at all. Its only
explicit argument is `user_id`. Instead of importing infrastructure dependencies, it
*imports capabilities* using the `#[uses(GetUser, FetchStorageObject)]` attribute. This
tells CGP that whatever context is used here must also implement the `GetUser` and
`FetchStorageObject` traits. Note that the identifier names in `#[uses]` are in
`CamelCase`, because `#[cgp_fn]` converts a function named `get_user` into a trait named
`GetUser` — following Rust's standard naming conventions for type-level constructs.

Critically, `get_user_profile_picture` does not need to know which fields `GetUser` or
`FetchStorageObject` depend on internally. It does not need to know that `GetUser`
requires a `database`, or that `FetchStorageObject` requires a `storage_client` and a
`bucket_id`. Those details are entirely self-contained within each individual function
definition. If `get_user` were later extended to also pull from a read replica or a
cache, `get_user_profile_picture` would not need to change at all.

This is one of CGP's central properties: **transitive dependencies are hidden**. Each
function declares only the capabilities it directly needs, not the full transitive closure
of everything its callees require.

### Seeing the Flexibility in Practice

The file concludes with a set of compile-time checks that confirm which contexts can use
which functions:

```rust
pub trait CheckGetUser: GetUser {}

impl CheckGetUser for App {}
impl CheckGetUser for MinimalApp {}
impl CheckGetUser for SmartApp {}
```

All three contexts — `App`, `MinimalApp`, and `SmartApp` — contain a `database: PgPool`
field, so all three automatically satisfy `GetUser`. No manual implementation is required
for any of them. The `#[cgp_fn]` machinery generates a blanket implementation that
covers every context with the required field.

However, `GetUserProfilePicture` requires both `GetUser` and `FetchStorageObject`, and
`FetchStorageObject` requires `storage_client` and `bucket_id`. `MinimalApp` only carries
a `database`, so it cannot satisfy `FetchStorageObject`, and thus cannot satisfy
`GetUserProfilePicture` either. This is reflected in the compile-time check:

```rust
pub trait CheckGetUserProfilePicture: GetUserProfilePicture {}

impl CheckGetUserProfilePicture for App {}
```

Only `App` is listed, because it is the only context in this chapter that contains all
three required fields. `MinimalApp` is correctly excluded — not because it lacks a
required implementation, but because it genuinely lacks the infrastructure to fulfill the
operation. The compiler enforces this at compile time, and no runtime errors are possible.

---

## Chapter 4: Generic Database Backends with `#[impl_generics]`

*Source file: cgp_fn_generic.rs*

The `get_user` implementation in Chapter 3 works for any context that contains a
`database: PgPool` field. However, `PgPool` is still a hardcoded concrete type. This
means that `EmbeddedApp` — which uses `SqlitePool` instead of `PgPool` — cannot use
that implementation, even though the database query logic is completely independent of
which database backend is used.

### Parameterizing the Database Type

The `#[impl_generics]` attribute lets you introduce additional generic type parameters
into the implementation that `#[cgp_fn]` generates. Here is the generic version of
`get_user`:

```rust
#[cgp_fn]
#[async_trait]
#[impl_generics(Db: Database)]
pub async fn get_user(
    &self,
    #[implicit] database: &Pool<Db>,
    user_id: &UserId,
) -> anyhow::Result<User>
where
    i64: sqlx::Type<Db>,
    for<'a> User: sqlx::FromRow<'a, Db::Row>,
    for<'a> i64: sqlx::Encode<'a, Db>,
    for<'a> <Db as sqlx::Database>::Arguments<'a>: sqlx::IntoArguments<'a, Db>,
    for<'a> &'a mut <Db as sqlx::Database>::Connection: sqlx::Executor<'a, Database = Db>,
{
    let user =
        sqlx::query_as("SELECT name, email, profile_picture_object_id FROM users WHERE id = $1")
            .bind(user_id.0 as i64)
            .fetch_one(database)
            .await?;

    Ok(user)
}
```

The `#[impl_generics(Db: Database)]` attribute introduces a generic type parameter `Db`
constrained to `sqlx::Database`. The implicit `database` parameter is now typed as
`&Pool<Db>` rather than `&PgPool`. The `where` clause captures the full set of
constraints that `sqlx` requires to compile a generic parameterized query: the types
involved must be encodable, the connection type must be an executor, and `User` must be
decodable from the database row type. These constraints are specific to `sqlx`'s generic
query API and are not unique to CGP.

The implementation body is unchanged. The same SQL query works whether `Db` is
`Postgres`, `Sqlite`, or any other `sqlx`-compatible database driver.

`EmbeddedApp` stores its database connection as a `SqlitePool`, which is an alias for
`Pool<Sqlite>`. Because the implicit parameter is now `&Pool<Db>`, CGP can match
`EmbeddedApp`'s `database` field with `Db = Sqlite`. All the `where` bounds are
verified at compile time for this specific choice — if a particular backend does not
support the required operations, the error will appear at the `impl CheckGetUser for
EmbeddedApp` line, pointing directly to the missing constraint.

The `fetch_storage_object` and `get_user_profile_picture` functions in this file are
identical to Chapter 3. The storage operation does not involve the database type at all,
so there is nothing to generalize there:

```rust
#[cgp_fn]
#[async_trait]
pub async fn fetch_storage_object(
    &self,
    #[implicit] storage_client: &Client,
    #[implicit] bucket_id: &str,
    object_id: &str,
) -> anyhow::Result<Vec<u8>> {
    // ... same implementation as before ...
}
```

The compile-time checks at the bottom of the file confirm the expanded coverage:

```rust
pub trait CheckGetUserProfilePicture: GetUserProfilePicture {}

impl CheckGetUserProfilePicture for App {}
impl CheckGetUserProfilePicture for EmbeddedApp {}
```

Both `App` (with `PgPool`) and `EmbeddedApp` (with `SqlitePool`) now satisfy
`GetUserProfilePicture` through the same single implementation — no duplication required.

### Trade-offs of Generic Implementations

The main cost of `#[impl_generics]` is increased complexity in the function signature.
The `where` clause for a generic `sqlx` query is substantially longer than the concrete
`PgPool` version. For a project that only ever targets PostgreSQL, the concrete version
from Chapter 3 is simpler, equally correct, and easier to read. You should reach for
`#[impl_generics]` when you know in advance that multiple concrete types need to share
the same logic, or when you want to support third-party extensions without requiring
changes to the core library.

---

## Chapter 5: Multiple Implementations with `#[cgp_impl]`

*Source file: cgp_impl.rs*

The `#[cgp_fn]` macro creates a single implementation for a given function. Every context
that satisfies the required fields uses the same code path. But some operations naturally
call for *multiple alternative implementations* — for example, storing objects in AWS S3
for one deployment, and in Google Cloud Storage for another. These alternatives have
completely different client APIs and cannot be unified into a single function body.

This is the scenario that `#[cgp_component]` and `#[cgp_impl]` are designed for. They
allow you to define a named interface with multiple competing providers, and let each
context declare which provider it uses at compile time.

### Defining a Component Trait

Instead of using `#[cgp_fn]` for `fetch_storage_object`, we declare an explicit
**component** using the `#[cgp_component]` macro:

```rust
#[async_trait]
#[cgp_component(StorageObjectFetcher)]
pub trait CanFetchStorageObject {
    async fn fetch_storage_object(&self, object_id: &str) -> anyhow::Result<Vec<u8>>;
}
```

`#[cgp_component(StorageObjectFetcher)]` generates two related constructs. The first is
the **consumer trait** `CanFetchStorageObject`, which defines the interface that
application code calls. A context implements `CanFetchStorageObject` to gain the ability
to call `self.fetch_storage_object(object_id)`. The second is the **provider trait**
`StorageObjectFetcher`, which is where alternative implementations will be written. The
provider trait has the same methods as the consumer, but the `Self` position is filled
by a named, dummy struct rather than by the actual application context.

### Writing Multiple Providers

With the component defined, we can write two independent providers — one for AWS S3 and
one for Google Cloud Storage:

```rust
#[cgp_impl(new FetchS3Object)]
impl StorageObjectFetcher {
    async fn fetch_storage_object(
        &self,
        #[implicit] storage_client: &Client,
        #[implicit] bucket_id: &str,
        object_id: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let output = storage_client
            .get_object()
            .bucket(bucket_id)
            .key(object_id)
            .send()
            .await?;

        let data = output.body.collect().await?.into_bytes().to_vec();
        Ok(data)
    }
}
```

`#[cgp_impl(new FetchS3Object)]` creates a named provider struct called `FetchS3Object`
and implements the `StorageObjectFetcher` provider trait on it. The `#[implicit]`
annotations work exactly as in `#[cgp_fn]`: `storage_client` and `bucket_id` will be
extracted from whatever context this provider is eventually used with. The `new` keyword
in the attribute means that the provider struct is declared here, rather than referring
to one defined elsewhere.

The Google Cloud provider follows the same structural pattern, but uses a completely
different client type and API:

```rust
#[cgp_impl(new FetchGCloudObject)]
impl StorageObjectFetcher {
    async fn fetch_storage_object(
        &self,
        #[implicit] storage_client: &Storage,
        #[implicit] bucket_id: &str,
        object_id: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let mut reader = storage_client
            .read_object(bucket_id, object_id)
            .send()
            .await?;

        let mut contents = Vec::new();
        while let Some(chunk) = reader.next().await.transpose()? {
            contents.extend_from_slice(&chunk);
        }

        Ok(contents)
    }
}
```

`FetchGCloudObject` requires a `storage_client: Storage` field (from the
`google-cloud-storage` crate), whereas `FetchS3Object` requires a `storage_client:
Client` field (from the `aws-sdk-s3` crate). The two providers are completely
independent. They know nothing about each other and can be defined in entirely separate
crates if needed. Each provider's dependency requirements are validated only when a
concrete context actually selects it.

### Wiring Contexts to Providers

With multiple providers available, each context must declare which one it intends to use.
This wiring is expressed using the `delegate_components!` macro:

```rust
delegate_components! {
    App {
        StorageObjectFetcherComponent: FetchS3Object,
    }
}

delegate_components! {
    GCloudApp {
        StorageObjectFetcherComponent: FetchGCloudObject,
    }
}
```

`App` holds an `aws_sdk_s3::Client` in its `storage_client` field, so it is wired to
`FetchS3Object`. `GCloudApp` holds a Google Cloud `Storage` client, so it is wired to
`FetchGCloudObject`. The wiring is verified at compile time in both directions: if a
context declares a provider it cannot satisfy — for example, if `GCloudApp` were
mistakenly wired to `FetchS3Object` — the compiler would report a clear error indicating
which implicit field is missing. There is no possibility of accidentally using the wrong
provider at runtime.

The `GCloudApp` context is defined in gcloud.rs:

```rust
#[derive(HasField)]
pub struct GCloudApp {
    pub database: PgPool,
    pub storage_client: Storage,
    pub bucket_id: String,
}
```

It is structurally identical to `App` except that its `storage_client` is a Google Cloud
`Storage` value rather than an AWS `Client`.

### The Top-Level Function Remains Unchanged

The `get_user_profile_picture` function in this chapter looks identical to the previous
chapters, with one small adjustment to the `#[uses]` declaration:

```rust
#[cgp_fn]
#[async_trait]
#[uses(GetUser, CanFetchStorageObject)]
pub async fn get_user_profile_picture(&self, user_id: &UserId) -> anyhow::Result<Option<RgbImage>> {
    let user = self.get_user(user_id).await?;

    if let Some(object_id) = user.profile_picture_object_id {
        let data = self.fetch_storage_object(&object_id).await?;
        let image = image::load_from_memory(&data)?.to_rgb8();

        Ok(Some(image))
    } else {
        Ok(None)
    }
}
```

The only difference from Chapter 3 is that `#[uses]` now lists `CanFetchStorageObject`
instead of `FetchStorageObject`, because the storage capability is now expressed through
a component trait rather than a plain `#[cgp_fn]`. The body of the function is
completely unchanged. It does not know, and does not need to know, whether the underlying
storage implementation uses S3, Google Cloud, or any future provider. That decision lives
entirely in `delegate_components!`.

The compile-time checks confirm that both contexts now satisfy the full feature:

```rust
pub trait CheckGetUserProfilePicture: GetUserProfilePicture {}

impl CheckGetUserProfilePicture for App {}
impl CheckGetUserProfilePicture for GCloudApp {}
```

### Trade-offs of Components

The `#[cgp_component]` and `#[cgp_impl]` approach is more powerful than a plain
`#[cgp_fn]`, but it also requires more ceremony. You need to define the consumer trait
separately, give each provider a name, and add a `delegate_components!` block for each
context that uses the component. For capabilities with a single natural implementation,
`#[cgp_fn]` remains the simpler and preferred choice.

Components become valuable when you genuinely need to select between multiple
implementations — for example, supporting different cloud storage backends, providing
mock implementations for integration testing, or allowing downstream libraries to supply
their own implementations of an interface your library defines. The key insight is that
the selection mechanism is entirely separate from the implementation logic, and the
caller (`get_user_profile_picture`) is completely insulated from the choice.

---

## Summary

This tutorial walked through the same feature — `get_user_profile_picture` — implemented
five different ways, each one addressing the limitations of the previous approach.

Plain functions in Chapter 1 were easy to understand but required every dependency to be
forwarded explicitly through every call site. Methods on a concrete `App` struct in
Chapter 2 cleaned up the call signatures, but locked every function to a single specific
context, preventing reuse across the different application contexts (`MinimalApp`,
`SmartApp`, `EmbeddedApp`) that a real project would naturally have.

Chapter 3 introduced `#[cgp_fn]` and `#[implicit]` arguments, which let us write a
function once and have it work automatically for any context that contains the required
fields. Transitive dependencies are hidden: `get_user_profile_picture` does not need to
know what fields `GetUser` or `FetchStorageObject` require internally. Chapter 4 extended
this further with `#[impl_generics]`, allowing the database type to become a generic
parameter so that `EmbeddedApp` (SQLite) and `App` (PostgreSQL) can share the same
`get_user` implementation.

Chapter 5 demonstrated that when a single implementation is not enough —
when you need to select between entirely different implementations, like AWS S3 and Google
Cloud Storage — CGP's `#[cgp_component]`, `#[cgp_impl]`, and `delegate_components!`
provide a clean, compile-time-safe wiring mechanism. Each context declares its own
provider, and the business logic that uses the component remains completely oblivious to
the choice.

Taken together, these tools let you write modular, reusable code that scales gracefully
with the complexity of your application, without forcing you to choose between
readability and flexibility.

## Further Reading

To learn more about Context-Generic Programming and the full capabilities of the CGP
framework, visit the official documentation at
[contextgeneric.dev](https://contextgeneric.dev).
