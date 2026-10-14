export const samples = [
  {name:'Scalar / checked i64',source:'x=3\nx=(x+1)*(x-1)+x*2\nprint(x)',inputs:[],values:[]},
  {name:'Quote scan / two awaits',source:'quotes=tools.load_quotes(batch)\ntotal=0\nfor q in quotes:\n    if q.unit_price_cents < limit:\n        total=total+q.unit_price_cents*q.quantity\nreceipt=tools.record_total(batch,total)\nprint(receipt)',
    inputs:[['batch','i64'],['limit','i64']],values:['2','100'],provider:{rows:2048,delay:100,price:'7',quantity:'3'},limits:{chunk:64}},
  {name:'Sequential buffers / chunk yield',source:'first=tools.load_quotes(batch)\ntotal=0\nfor q in first:\n    total=total+q.unit_price_cents*q.quantity\nsecond=tools.load_quotes(batch+1)\nfor q in second:\n    total=total+q.unit_price_cents\nreceipt=tools.record_total(batch,total)\nprint(receipt)',
    inputs:[['batch','i64']],values:['2'],provider:{rows:4096,delay:100,price:'7',quantity:'3'},limits:{chunk:64}},
];
