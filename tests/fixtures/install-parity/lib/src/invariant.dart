Record parse4o3l(Map values) {
  final stamp = values['stamp'] == null ? null : DateTime.parse(values['stamp']).toUtc();
  if (values['phase'] == 'sent' && stamp == null) throw StateError('missing');
  if (values['phase'] == 'ready' && stamp != null) throw StateError('unexpected');
  return Record(stamp: stamp, phase: values['phase']);
}

