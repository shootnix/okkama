package okkama.rt;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.*;

/** Immutable owned DB cells, shared schema lookup; same contract as Rust runtime/db. */
public final class DbNatives {
    private DbNatives() {}
    static final class Cell {
        final int kind;
        final Object value;
        Cell(int kind, Object value) { this.kind=kind; this.value=value; }
    }
    static final class Schema {
        final Map<String, Integer> positions=new HashMap<>();
        Schema(List<?> names) {
            if(names.size()>1024) throw invalid("db_schema",1);
            for(int i=0;i<names.size();i++) {
                String name=(String)names.get(i);
                if(name.getBytes(StandardCharsets.UTF_8).length>4096) throw invalid("db_schema",1);
                positions.put(name,positions.containsKey(name)?-2:i);
            }
        }
    }
    private static Failure invalid(String name,int arity) { return NativeFailure.raise("db","invalid_input","runtime/db@1::"+name+"/"+arity,"invalid database cell or projection"); }
    private static Object value(Object c,int kind,String name) { Cell cell=(Cell)c;if(cell.kind!=kind)throw invalid(name,1);return cell.value; }
    public static Object runtime_db__db_null(){return new Cell(0,null);}
    public static Object runtime_db__db_boolean(boolean v){return new Cell(1,v);}
    public static Object runtime_db__db_integer(String v){
        if(v.length()>4096 || !v.matches("-?[0-9]+"))throw invalid("db_integer",1);
        return new Cell(2,new BigInteger(v));
    }
    public static Object runtime_db__db_floating(Double v){if(!Double.isFinite(v))throw invalid("db_floating",1);return new Cell(3,v);}
    public static Object runtime_db__db_decimal(String v){if(v.length()>4096 || !ValueNatives.runtime_decimal__decimal_valid(v))throw invalid("db_decimal",1);return new Cell(4,v);}
    public static Object runtime_db__db_text(String v){if(v.getBytes(StandardCharsets.UTF_8).length>8388608)throw invalid("db_text",1);return new Cell(5,v);}
    public static Object runtime_db__db_bytes(List<?> v){if(v.size()>8388608)throw invalid("db_bytes",1);return new Cell(6,Collections.unmodifiableList(new ArrayList<Object>(v)));}
    public static BigInteger runtime_db__db_kind(Object c){return BigInteger.valueOf(((Cell)c).kind);}
    public static boolean runtime_db__db_boolean_value(Object c){return (Boolean)value(c,1,"db_boolean_value");}
    public static String runtime_db__db_integer_value(Object c){return value(c,2,"db_integer_value").toString();}
    public static boolean runtime_db__db_int_valid(Object c){return ((Cell)c).kind==2 && ((BigInteger)((Cell)c).value).bitLength()<=63;}
    public static BigInteger runtime_db__db_int_value(Object c){if(!runtime_db__db_int_valid(c))throw invalid("db_int_value",1);return (BigInteger)((Cell)c).value;}
    public static Double runtime_db__db_floating_value(Object c){return (Double)value(c,3,"db_floating_value");}
    public static String runtime_db__db_decimal_value(Object c){return (String)value(c,4,"db_decimal_value");}
    public static String runtime_db__db_text_value(Object c){return (String)value(c,5,"db_text_value");}
    @SuppressWarnings("unchecked")
    public static List<Object> runtime_db__db_bytes_value(Object c){return (List<Object>)value(c,6,"db_bytes_value");}
    public static Object runtime_db__db_schema(List<?> names){return new Schema(names);}
    public static BigInteger runtime_db__db_position(Object schema,String name){return BigInteger.valueOf(((Schema)schema).positions.getOrDefault(name,-1));}
    public static BigInteger runtime_db__db_cells_bytes(List<?> cells){
        long size=0;
        for(Object o:cells){Cell c=(Cell)o;
            if(c.kind==1)size++;
            else if(c.kind==2 || c.kind==4)size+=c.value.toString().length();
            else if(c.kind==3)size+=Floats.text((Double)c.value).length();
            else if(c.kind==5)size+=((String)c.value).getBytes(StandardCharsets.UTF_8).length;
            else if(c.kind==6)size+=((List<?>)c.value).size();
            if(size>8388608)break;
        }
        return BigInteger.valueOf(size);
    }
    static final class Capture { final int kind; boolean started,faulted,poisoned,sealed;Cell cell;Capture(int k){kind=k;} }
    public static Object runtime_db__db_capture(BigInteger kind){if(kind.signum()<0 || kind.compareTo(BigInteger.valueOf(6))>0)throw invalid("db_capture",1);return new Capture(kind.intValue());}
    public static BigInteger runtime_db__db_capture_emit(Object h,BigInteger kind){Capture c=(Capture)h;synchronized(c){
        if(c.sealed)return BigInteger.ONE;
        if(c.faulted || c.started || !kind.equals(BigInteger.valueOf(c.kind))){c.faulted=true;c.poisoned=true;return BigInteger.valueOf(2);}
        c.started=true;c.faulted=true;return BigInteger.ZERO;
    }}
    public static boolean runtime_db__db_capture_publish(Object h,Object value){Capture c=(Capture)h;Cell cell=(Cell)value;synchronized(c){
        boolean valid=!c.sealed && !c.poisoned && c.started && c.cell==null && cell.kind==c.kind;
        if(valid){c.cell=cell;c.faulted=false;}else if(!c.sealed)c.faulted=true;return valid;
    }}
    public static boolean runtime_db__db_capture_complete(Object h,boolean seal){Capture c=(Capture)h;synchronized(c){boolean valid=c.started && !c.faulted && c.cell!=null;if(valid && seal)c.sealed=true;return valid;}}
    public static Object runtime_db__db_capture_value(Object h){Capture c=(Capture)h;synchronized(c){if(c.faulted || c.cell==null)throw invalid("db_capture_value",1);return c.cell;}}
}
