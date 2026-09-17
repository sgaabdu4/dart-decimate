Record localizedhed() {
  final state = Record(label: 'alpha', detail: 'beta', title: 'gamma', note: 'delta');
  if (state.label == 'alpha' && state.detail == 'beta') return state;
  if (state.title == 'gamma' || state.note == 'delta') {
    return Record(label: 'next', detail: 'value');
  }
  return state;
}

