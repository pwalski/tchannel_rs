###### BUILD ######
# syntax=docker/dockerfile:1
FROM maven:3-openjdk-17-slim as maven_build
WORKDIR /build

COPY src ./src
COPY pom.xml .

RUN mvn -B clean package

###### RUN ######
FROM openjdk:17-slim
WORKDIR /app

ENV JAR_NAME=tchannel-jvm-server.jar

COPY --from=maven_build /build/target/$JAR_NAME $JAR_NAME

ENTRYPOINT java -jar $JAR_NAME
