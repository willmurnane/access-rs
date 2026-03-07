ARG fuzz_target
ARG flavor=vanilla
FROM docker.io/rustlang/rust:nightly AS result
RUN cargo install cargo-afl \
    && rustup default nightly
RUN apt-get update && apt-get install -y clang llvm-dev cmake git maven
RUN cargo afl config --build --force
WORKDIR /app
COPY access/ access
COPY afl-target/ afl-target
COPY libfuzz libfuzz
COPY Cargo.* ./
ENV RUSTFLAGS="-C link-arg=-Wl,--allow-multiple-definition"
# Build with AFL instrumentation
ARG fuzz_target
ARG flavor
ENV FLAVOR=${flavor}
ARG TARGETARCH
ARG TARGETOS
ENV J4RS_BASE_PATH=/app/result/deps
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=build-${fuzz_target}-${TARGETARCH}-${TARGETOS} \
    --mount=type=cache,target=/app/target,id=build-${fuzz_target}-${TARGETARCH}-${TARGETOS} \
    set -x; \
    if [ ${FLAVOR} = asan ]; then export RUSTFLAGS="$RUSTFLAGS -Zsanitizer=address"; fi && \
    cargo afl build --release --target=$(uname -m)-unknown-linux-gnu --bin afl-access-${fuzz_target} && \
    mkdir -p /app/result && \
    mv /app/target/$(uname -m)-unknown-linux-gnu/release/afl-access-${fuzz_target} /app/result/fuzz-target && \
    mkdir -p target/release/deps /app/result/jassets && \
    # Special case for the java target: we need the deps and jassets folders to be copied to the final image.
    find / -name 'j4rs*.jar' | xargs -I{} cp {} /app/result/jassets/ && \
    if [ ${fuzz_target} = java ]; then [ -f /app/result/jassets/j4rs*.jar ] || { \
    echo "No j4rs jar found in deps dir:"; \
    find /app/result/deps; \
    echo "Target dir:"; \
    ls -l target/$(uname -m)-unknown-linux-gnu/release; \
    echo "j4rs jars anywhere in filesystem:"; \
    find / -name 'j4rs*.jar'; \
    exit 1; \
    }; fi

# Special dependency stage for 'java'.
FROM ubuntu:latest AS access
WORKDIR /result
ENV M2_HOME=/opt/maven MAVEN_HOME=/opt/maven PATH=/opt/maven/bin:${PATH}
ARG fuzz_target

# Do everything in a single stage gated by fuzz_target, so that other targets can quickly skip it.
RUN if [ ${fuzz_target} != java ]; then exit 0; fi && \
    apt-get update && \
    apt-get install -y --no-install-recommends curl default-jdk git && \
    mkdir -p /opt/maven && \
    cd /opt/maven && \
    curl -Lo maven.tar.gz https://dlcdn.apache.org/maven/maven-3/3.9.14/binaries/apache-maven-3.9.14-bin.tar.gz && \
    tar --strip-components=1 -zxf maven.tar.gz && \
    mkdir -p /build && \
    cd /build && \
    git clone https://github.com/apache/accumulo-access.git . && \
    mvn package -DskipTests && \
    mv /build/modules/core/target/accumulo-access-core-1.0.0-beta2-SNAPSHOT.jar /result/access.jar

FROM eclipse-temurin:25
ARG fuzz_target
ARG flavor
COPY --from=result  /app/result/ /
COPY --from=access /result/* /deps/
COPY ${fuzz_target}-in /in
ENV JAR_LOCATION=/deps/access.jar
ENV J4RS_BASE_PATH=/
RUN echo $JAR_LOCATION && \
    echo "Deps:" && \
    find /deps -name '*.jar'; \
    echo "Jassets:" && \
    find /jassets -name '*.jar'; \
    # Fails on arm64 when building with qemu.
    if [ ${flavor} != asan ]; then \
    echo | /fuzz-target; \
    fi

ENV AFL_NO_AFFINITY=1
