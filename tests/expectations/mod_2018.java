import com.sun.jna.*;
import com.sun.jna.ptr.*;

enum BindingsSingleton {
  INSTANCE;
  final Bindings lib = Native.load("mod_2018", Bindings.class);
}

interface Bindings extends Library {
  Bindings INSTANCE = BindingsSingleton.INSTANCE.lib;

  /* Unsupported literal for constant EXPORT_ME_TOO */



  @Structure.FieldOrder({"val"})
  class ExportMe extends Structure implements Structure.ByValue {
    public ExportMe() {
      super();
    }

    public ExportMe(Pointer p) {
      super(p);
    }

    public long val;

  }

  @Structure.FieldOrder({"val"})
  class ExportMeByReference extends Structure implements Structure.ByReference {
    public ExportMeByReference() {
      super();
    }

    public ExportMeByReference(Pointer p) {
      super(p);
    }

    public long val;

  }



  @Structure.FieldOrder({"val"})
  class ExportMe2 extends Structure implements Structure.ByValue {
    public ExportMe2() {
      super();
    }

    public ExportMe2(Pointer p) {
      super(p);
    }

    public long val;

  }

  @Structure.FieldOrder({"val"})
  class ExportMe2ByReference extends Structure implements Structure.ByReference {
    public ExportMe2ByReference() {
      super();
    }

    public ExportMe2ByReference(Pointer p) {
      super(p);
    }

    public long val;

  }


  void export_me(ExportMeByReference val);

  void export_me_2(ExportMe2ByReference arg0);

  void from_really_nested_mod();

}