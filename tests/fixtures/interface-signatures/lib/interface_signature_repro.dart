abstract class StoragePort {
  Future<Map<String, Object?>> writeStream({
    required Stream<List<int>> stream,
    required String fileExtension,
    required bool? useMemory,
    required Object keyStore,
    required Object idGenerator,
    required Object Function(int) getContainer,
    int? chunkSize,
  });
}

class StorageAdapter implements StoragePort {
  @override
  Future<Map<String, Object?>> writeStream({
    required Stream<List<int>> stream,
    required String fileExtension,
    required bool? useMemory,
    required Object keyStore,
    required Object idGenerator,
    required Object Function(int) getContainer,
    int? chunkSize,
  }) async => {};
}
